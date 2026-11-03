use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const SCORE_LINK_PARAMS_SUFFIX: &str =
    "score/mw/com/example/vehicle-dynamics-example/vehicle-dynamics-example-0.params";

fn main() {
    println!("cargo:rerun-if-env-changed=SCORE_COM_BAZEL_BIN");
    println!("cargo:rerun-if-env-changed=SCORE_COM_BAZEL_CACHE_ROOTS");
    println!("cargo:rerun-if-env-changed=SCORE_COM_LINK_PARAMS");
    println!("cargo:rerun-if-env-changed=SCORE_COM_VEHICLE_GEN_LIB_DIR");

    let Some((bazel_bin, params_path)) = resolve_score_paths() else {
        println!(
            "cargo:warning=S-Core link params not found; set SCORE_COM_LINK_PARAMS and SCORE_COM_BAZEL_BIN before building score-lola binaries"
        );
        return;
    };

    // Prefer the exact Bazel link params because they contain the complete set
    // of native LoLa library search paths and static libraries for this target.
    if link_from_bazel_params(&bazel_bin, &params_path) {
        println!("cargo:rustc-link-lib=dylib=stdc++");
        println!("cargo:rustc-link-lib=dylib=rt");
        println!("cargo:rustc-link-lib=dylib=atomic");
        println!("cargo:rustc-link-lib=dylib=acl");
        return;
    }

    require_link_dir(
        bazel_bin.join("score/mw/com/impl/rust/com-api/com-api-ffi-lola"),
        "registry_bridge_macro_cpp",
    );
    require_link_dir(vehicle_gen_dir(&bazel_bin), "vehicle_gen_cpp");

    println!("cargo:rustc-link-lib=dylib=registry_bridge_macro_cpp");
    println!("cargo:rustc-link-lib=dylib=vehicle_gen_cpp");
    println!("cargo:rustc-link-lib=dylib=stdc++");
}

fn link_from_bazel_params(bazel_bin: &Path, params_path: &Path) -> bool {
    let params = match fs::read_to_string(&params_path) {
        Ok(params) => params,
        Err(error) => {
            if env::var_os("SCORE_COM_BAZEL_BIN").is_some() {
                panic!(
                    "failed to read S-Core link params at {}: {error}. \
                     Rebuild the S-Core vehicle-dynamics-example target and set \
                     SCORE_COM_LINK_PARAMS to its vehicle-dynamics-example-0.params file. \
                     If SCORE_COM_BAZEL_BIN points to a stale bazel-bin symlink, use `bazel info bazel-bin` from the S-Core communication checkout and export that path instead.",
                    params_path.display()
                );
            }
            return false;
        }
    };
    let Some(score_build_root) = score_build_root(&bazel_bin, &params_path) else {
        panic!(
            "failed to locate S-Core Bazel execroot for {}. \
             Set SCORE_COM_LINK_PARAMS to an absolute params file under the active Bazel output tree.",
            params_path.display()
        );
    };

    println!("cargo:rerun-if-changed={}", params_path.display());
    link_dir(vehicle_gen_dir(bazel_bin));

    let mut lines = params.lines().map(str::trim).filter(|line| !line.is_empty()).peekable();
    while let Some(line) = lines.next() {
        if let Some(path) = line.strip_prefix("-Lnative=") {
            link_dir(score_build_root.join(path));
        } else if let Some(library) = line.strip_prefix("-lstatic=") {
            println!("cargo:rustc-link-lib=static={library}");
        } else if let Some(argument) = line.strip_prefix("-Clink-arg=") {
            handle_link_arg(argument);
        } else if line == "-Clink-arg" {
            if let Some(argument) = lines.next() {
                handle_link_arg(argument);
            }
        } else if let Some(argument) = line.strip_prefix("--codegen=link-arg=") {
            if matches!(argument, "-lpthread" | "-lrt" | "-lm" | "-ldl") {
                println!("cargo:rustc-link-arg={argument}");
            }
        }
    }

    true
}

fn resolve_score_paths() -> Option<(PathBuf, PathBuf)> {
    // Explicit paths are the stable path for CI. Auto-discovery keeps local
    // demos ergonomic when the S-Core Bazel cache already contains the artifacts.
    if let Some(bazel_bin) = env::var_os("SCORE_COM_BAZEL_BIN").map(PathBuf::from) {
        let params_path = env::var_os("SCORE_COM_LINK_PARAMS")
            .map(PathBuf::from)
            .unwrap_or_else(|| bazel_bin.join(SCORE_LINK_PARAMS_SUFFIX));
        return Some((bazel_bin, params_path));
    }

    let params_path = env::var_os("SCORE_COM_LINK_PARAMS")
        .map(PathBuf::from)
        .or_else(discover_score_link_params)?;
    let bazel_bin = bazel_bin_from_params(&params_path)?;

    println!(
        "cargo:warning=Auto-discovered S-Core link params at {}; export SCORE_COM_LINK_PARAMS/SCORE_COM_BAZEL_BIN to make this explicit",
        params_path.display()
    );
    Some((bazel_bin, params_path))
}

fn bazel_bin_from_params(params_path: &Path) -> Option<PathBuf> {
    let params_path = params_path.to_str()?;
    let bazel_bin = params_path.strip_suffix(SCORE_LINK_PARAMS_SUFFIX)?;
    Some(PathBuf::from(bazel_bin.trim_end_matches('/')))
}

fn discover_score_link_params() -> Option<PathBuf> {
    // Scan configured/current-user Bazel caches only; never depend on a
    // committed machine-specific checkout or username.
    bazel_cache_roots()
        .into_iter()
        .filter(|path| path.is_dir())
        .filter_map(|path| newest_score_link_params(&path))
        .max_by_key(|path| fs::metadata(path).and_then(|metadata| metadata.modified()).ok())
}

fn bazel_cache_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(value) = env::var_os("SCORE_COM_BAZEL_CACHE_ROOTS") {
        roots.extend(env::split_paths(&value));
    }
    if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
        roots.push(home.join(".cache/bazel"));
    }
    if let Some(cache_home) = env::var_os("XDG_CACHE_HOME").map(PathBuf::from) {
        roots.push(cache_home.join("bazel"));
    }

    roots
}

fn newest_score_link_params(root: &Path) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    let mut newest = None;
    while let Some(path) = stack.pop() {
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.components().any(|component| component.as_os_str() == "sandbox") {
                    continue;
                }
                stack.push(path);
            } else if path.ends_with(SCORE_LINK_PARAMS_SUFFIX) {
                newest = newest
                    .into_iter()
                    .chain([path])
                    .max_by_key(|path| fs::metadata(path).and_then(|metadata| metadata.modified()).ok());
            }
        }
    }
    newest
}

fn handle_link_arg(argument: &str) {
    if argument.starts_with("bazel-out/") && argument.ends_with(".lo") {
        return;
    }

    if argument == "-Wl,--whole-archive" || argument == "-Wl,--no-whole-archive" {
        return;
    }
}

fn score_build_root(bazel_bin: &Path, params_path: &Path) -> Option<PathBuf> {
    bazel_bin
        .ancestors()
        .chain(params_path.ancestors())
        .find(|candidate| candidate.join("bazel-out").is_dir())
        .map(Path::to_path_buf)
}

fn vehicle_gen_dir(bazel_bin: &Path) -> PathBuf {
    env::var_os("SCORE_COM_VEHICLE_GEN_LIB_DIR").map(PathBuf::from).unwrap_or_else(|| {
        bazel_bin.join("score/mw/com/example/vehicle-dynamics-example/com-api-gen")
    })
}

fn link_dir(path: PathBuf) {
    println!("cargo:rustc-link-search=native={}", path.display());
}

fn require_link_dir(path: PathBuf, library: &str) {
    if !path.exists() {
        panic!(
            "S-Core Bazel output for {library} was not found at {}. Set SCORE_COM_BAZEL_BIN to the output of `bazel info bazel-bin` from the S-Core communication checkout after building the vehicle-dynamics example.",
            path.display()
        );
    }
    link_dir(path);
}