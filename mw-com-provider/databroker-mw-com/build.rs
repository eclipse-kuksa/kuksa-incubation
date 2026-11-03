/********************************************************************************
 * Copyright (c) 2026 Contributors to the Eclipse Foundation
 *
 * See the NOTICE file(s) distributed with this work for additional
 * information regarding copyright ownership.
 *
 * This program and the accompanying materials are made available under the
 * terms of the Apache License Version 2.0 which is available at
 * https://www.apache.org/licenses/LICENSE-2.0
 *
 * SPDX-License-Identifier: Apache-2.0
 ********************************************************************************/

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SCORE_LINK_PARAMS_SUFFIX: &str =
    "score/mw/com/example/vehicle-dynamics-example/vehicle-dynamics-example-0.params";

fn main() {
    println!("cargo:rerun-if-env-changed=SCORE_COM_BAZEL_BIN");
    println!("cargo:rerun-if-env-changed=SCORE_COM_BAZEL_CACHE_ROOTS");
    println!("cargo:rerun-if-env-changed=SCORE_COM_LINK_PARAMS");
    println!("cargo:rerun-if-env-changed=SCORE_COM_VEHICLE_GEN_LIB_DIR");

    let Some((bazel_bin, params_path)) = resolve_score_paths() else {
        println!(
            "cargo:warning=S-Core link params not found; set SCORE_COM_LINK_PARAMS and SCORE_COM_BAZEL_BIN before building databroker-mw-com"
        );
        return;
    };

    // Some S-Core archives are emitted as alwayslink inputs in Bazel params.
    // Merge them into a Cargo-visible archive so the final binary keeps the
    // generated registration symbols that ordinary dead-code elimination drops.
    let alwayslink_archives = collect_alwayslink_archives(&bazel_bin, &params_path);
    if let Some(archive_path) = create_alwayslink_archive(&alwayslink_archives) {
        if let Some(archive_dir) = archive_path.parent() {
            println!("cargo:rustc-link-search=native={}", archive_dir.display());
        }
        println!("cargo:rustc-link-lib=static:+whole-archive=score_com_alwayslink");
    } else if !alwayslink_archives.is_empty() {
        println!("cargo:rustc-link-arg=-Wl,--whole-archive");
        for archive in alwayslink_archives {
            println!("cargo:rustc-link-arg={}", archive.display());
        }
        println!("cargo:rustc-link-arg=-Wl,--no-whole-archive");
    }
}

fn collect_alwayslink_archives(bazel_bin: &Path, params_path: &Path) -> Vec<PathBuf> {
    let params = fs::read_to_string(&params_path).unwrap_or_else(|error| {
        panic!(
            "failed to read S-Core link params at {}: {error}. \
             Rebuild the S-Core vehicle-dynamics-example target and set \
             SCORE_COM_LINK_PARAMS to its vehicle-dynamics-example-0.params file. \
             If SCORE_COM_BAZEL_BIN points to a stale bazel-bin symlink, use `bazel info bazel-bin` from the S-Core communication checkout and export that path instead.",
            params_path.display()
        )
    });
    let Some(score_build_root) = score_build_root(bazel_bin, &params_path) else {
        panic!(
            "failed to locate S-Core Bazel execroot for {}. \
             Set SCORE_COM_LINK_PARAMS to an absolute params file under the active Bazel output tree.",
            params_path.display()
        );
    };

    println!("cargo:rerun-if-changed={}", params_path.display());

    let mut archives = Vec::new();
    let mut lines = params
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .peekable();
    while let Some(line) = lines.next() {
        if let Some(argument) = line.strip_prefix("-Clink-arg=") {
            collect_link_arg(&score_build_root, argument, &mut archives);
        } else if line == "-Clink-arg" {
            if let Some(argument) = lines.next() {
                collect_link_arg(&score_build_root, argument, &mut archives);
            }
        }
    }

    archives
}

fn resolve_score_paths() -> Option<(PathBuf, PathBuf)> {
    // Explicit paths are preferred for CI and reproducible developer builds.
    // Cache scanning below is only a local convenience after the S-Core example
    // has already been built once with Bazel.
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
    // Bazel output roots are intentionally discovered from user/cache locations,
    // not from committed absolute paths. This keeps the workspace portable.
    bazel_cache_roots()
        .into_iter()
        .filter(|path| path.is_dir())
        .filter_map(|path| newest_score_link_params(&path))
        .max_by_key(|path| {
            fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .ok()
        })
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
                if path
                    .components()
                    .any(|component| component.as_os_str() == "sandbox")
                {
                    continue;
                }
                stack.push(path);
            } else if path.ends_with(SCORE_LINK_PARAMS_SUFFIX) {
                newest = newest.into_iter().chain([path]).max_by_key(|path| {
                    fs::metadata(path)
                        .and_then(|metadata| metadata.modified())
                        .ok()
                });
            }
        }
    }
    newest
}

fn collect_link_arg(score_build_root: &Path, argument: &str, archives: &mut Vec<PathBuf>) {
    if argument.starts_with("bazel-out/") && argument.ends_with(".lo") {
        archives.push(score_build_root.join(argument));
    }
}

fn score_build_root(bazel_bin: &Path, params_path: &Path) -> Option<PathBuf> {
    bazel_bin
        .ancestors()
        .chain(params_path.ancestors())
        .find(|candidate| candidate.join("bazel-out").is_dir())
        .map(Path::to_path_buf)
}

fn create_alwayslink_archive(archives: &[PathBuf]) -> Option<PathBuf> {
    if archives.is_empty() {
        return None;
    }

    let out_dir = env::var_os("OUT_DIR").map(PathBuf::from)?;
    let archive_path = out_dir.join("libscore_com_alwayslink.a");
    let _ = fs::remove_file(&archive_path);
    let mut script = format!("CREATE {}\n", archive_path.display());
    for (index, archive) in archives.iter().enumerate() {
        println!("cargo:rerun-if-changed={}", archive.display());
        let local_archive = out_dir.join(format!("score_com_alwayslink_part_{index}.a"));
        fs::copy(archive, &local_archive).ok()?;
        script.push_str(&format!("ADDLIB {}\n", local_archive.display()));
    }
    script.push_str("SAVE\nEND\n");

    let mut command = Command::new("ar")
        .arg("-M")
        .stdin(Stdio::piped())
        .spawn()
        .ok()?;
    command.stdin.as_mut()?.write_all(script.as_bytes()).ok()?;
    let status = command.wait().ok()?;
    status.success().then_some(archive_path)
}
