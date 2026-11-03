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
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const DEFAULT_MODEL: &str = "models/vehicle_dynamics.json";
const DEFAULT_TEMPLATES: &str = "templates/reference-codegen";
const DEFAULT_SCORE_TEMPLATE_CONFIG: &str = "templates/score-com-api/config.yaml";
const DEFAULT_SCORE_TEMPLATE_CACHE: &str = "target/xtask/score-com-api-templates";
const DEFAULT_SCORE_OUT: &str = "generated/score-com-api";
const DEFAULT_OUT: &str = "generated";
const DEFAULT_PREFIX: &str = "mw_com";
const DEFAULT_CODEGEN_RELATIVE: &str = "../../vsps_sdk_score_config_codegen-main";
const DEFAULT_CODEGEN_CACHE: &str = "target/xtask/vsps_sdk_score_config_codegen-main";

fn main() -> Result<()> {
    let mut args = env::args_os();
    let _program = args.next();
    let Some(command) = args.next() else {
        print_usage();
        return Err("missing xtask command".into());
    };

    match command.to_string_lossy().as_ref() {
        "codegen" => codegen(CodegenArgs::parse(args)?, false),
        "verify-codegen" => codegen(CodegenArgs::parse(args)?, true),
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("unknown xtask command: {other}").into()),
    }
}

#[derive(Debug)]
struct CodegenArgs {
    model: PathBuf,
    templates: PathBuf,
    out: PathBuf,
    prefix: String,
    score_templates: Option<PathBuf>,
    score_out: PathBuf,
    score_prefix: String,
    codegen_root: PathBuf,
    codegen_git_url: Option<String>,
    codegen_ref: Option<String>,
}

impl CodegenArgs {
    fn parse(args: impl Iterator<Item = OsString>) -> Result<Self> {
        let provider_root = provider_root()?;
        let explicit_codegen_root = env::var_os("MW_COM_CODEGEN_ROOT").map(PathBuf::from);
        let mut parsed = Self {
            model: provider_root.join(DEFAULT_MODEL),
            templates: provider_root.join(DEFAULT_TEMPLATES),
            out: provider_root.join(DEFAULT_OUT),
            prefix: DEFAULT_PREFIX.to_string(),
            score_templates: env::var_os("MW_COM_SCORE_TEMPLATES").map(PathBuf::from),
            score_out: env::var_os("MW_COM_SCORE_OUT")
                .map(PathBuf::from)
                .unwrap_or_else(|| provider_root.join(DEFAULT_SCORE_OUT)),
            score_prefix: env::var("MW_COM_SCORE_PREFIX")
                .unwrap_or_else(|_| DEFAULT_PREFIX.to_string()),
            codegen_root: explicit_codegen_root
                .unwrap_or_else(|| provider_root.join(DEFAULT_CODEGEN_RELATIVE)),
            codegen_git_url: env::var("MW_COM_CODEGEN_GIT_URL").ok(),
            codegen_ref: env::var("MW_COM_CODEGEN_GIT_REF").ok(),
        };

        let mut codegen_root_overridden = env::var_os("MW_COM_CODEGEN_ROOT").is_some();

        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            let arg = arg.to_string_lossy();
            match arg.as_ref() {
                "--model" => parsed.model = required_path_value(&mut args, "--model")?,
                "--templates" => parsed.templates = required_path_value(&mut args, "--templates")?,
                "--out" => parsed.out = required_path_value(&mut args, "--out")?,
                "--prefix" => parsed.prefix = required_string_value(&mut args, "--prefix")?,
                "--score-templates" => {
                    parsed.score_templates =
                        Some(required_path_value(&mut args, "--score-templates")?)
                }
                "--score-out" => parsed.score_out = required_path_value(&mut args, "--score-out")?,
                "--score-prefix" => {
                    parsed.score_prefix = required_string_value(&mut args, "--score-prefix")?
                }
                "--codegen-root" => {
                    parsed.codegen_root = required_path_value(&mut args, "--codegen-root")?;
                    codegen_root_overridden = true;
                }
                "--codegen-git-url" => {
                    parsed.codegen_git_url =
                        Some(required_string_value(&mut args, "--codegen-git-url")?)
                }
                "--codegen-ref" => {
                    parsed.codegen_ref = Some(required_string_value(&mut args, "--codegen-ref")?)
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                unknown => return Err(format!("unknown codegen option: {unknown}").into()),
            }
        }

        parsed.model = absolutize(&parsed.model)?;
        parsed.templates = absolutize(&parsed.templates)?;
        parsed.out = absolutize(&parsed.out)?;
        parsed.score_templates = parsed
            .score_templates
            .as_deref()
            .map(absolutize)
            .transpose()?;
        parsed.score_out = absolutize(&parsed.score_out)?;
        parsed.codegen_root = absolutize(&parsed.codegen_root)?;

        if !codegen_root_overridden && !parsed.codegen_root.exists() {
            let cache_root = provider_root.join(DEFAULT_CODEGEN_CACHE);
            if cache_root.exists() || parsed.codegen_git_url.is_some() {
                parsed.codegen_root = cache_root;
            }
        }

        Ok(parsed)
    }
}

fn required_path_value(
    args: &mut std::iter::Peekable<impl Iterator<Item = OsString>>,
    option: &str,
) -> Result<PathBuf> {
    args.next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing value for {option}").into())
}

fn required_string_value(
    args: &mut std::iter::Peekable<impl Iterator<Item = OsString>>,
    option: &str,
) -> Result<String> {
    args.next()
        .map(|value| value.to_string_lossy().into_owned())
        .ok_or_else(|| format!("missing value for {option}").into())
}

fn provider_root() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "failed to resolve provider root from xtask manifest".into())
}

fn absolutize(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()?.join(path))
    }
}

fn codegen(args: CodegenArgs, verify: bool) -> Result<()> {
    ensure_file(&args.model, "model")?;
    ensure_dir(&args.templates, "template directory")?;
    ensure_codegen_checkout(&args)?;

    run_codegen(
        "mw::com provider artifacts",
        &args.codegen_root,
        &args.model,
        &args.templates,
        &args.out,
        &args.prefix,
    )?;

    validate_generated_json(&args.out.join("mw_com_provider_config.json"))?;
    format_generated_rust(&args.out)?;

    if let Some(score_templates) = &args.score_templates {
        let score_templates = score_template_dir(score_templates)?;
        run_codegen(
            "S-Core mw::com API artifacts",
            &args.codegen_root,
            &args.model,
            &score_templates,
            &args.score_out,
            &args.score_prefix,
        )?;
        format_generated_rust(&args.score_out)?;
    }

    if verify {
        verify_generated_is_clean(&args.out)?;
        if args.score_templates.is_some() {
            verify_generated_is_clean(&args.score_out)?;
        }
    }

    Ok(())
}

fn run_codegen(
    label: &str,
    codegen_root: &Path,
    model: &Path,
    templates: &Path,
    out: &Path,
    prefix: &str,
) -> Result<()> {
    fs::create_dir_all(out)?;

    println!("Generating {label}");
    println!("  model: {}", model.display());
    println!("  templates: {}", templates.display());
    println!("  output: {}", out.display());
    println!("  prefix: {prefix}");
    println!("  codegen: {}", codegen_root.display());

    run(Command::new("bazel")
        .current_dir(codegen_root)
        .arg("run")
        .arg("//:codegen")
        .arg("--")
        .arg("-t")
        .arg(templates)
        .arg("-i")
        .arg(model)
        .arg("--prefix")
        .arg(prefix)
        .arg("-o")
        .arg(out))
}

fn score_template_dir(source: &Path) -> Result<PathBuf> {
    ensure_dir(source, "S-Core template directory")?;

    if source.join("config.yaml").is_file() {
        return Ok(source.to_path_buf());
    }

    let provider_root = provider_root()?;
    let cache = provider_root.join(DEFAULT_SCORE_TEMPLATE_CACHE);
    let config = provider_root.join(DEFAULT_SCORE_TEMPLATE_CONFIG);
    ensure_file(&config, "S-Core template config")?;

    if cache.exists() {
        fs::remove_dir_all(&cache)?;
    }
    fs::create_dir_all(&cache)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            fs::copy(&path, cache.join(entry.file_name()))?;
        }
    }
    fs::copy(&config, cache.join("config.yaml"))?;
    reject_legacy_score_templates(&cache)?;

    Ok(cache)
}

fn reject_legacy_score_templates(templates: &Path) -> Result<()> {
    let mut legacy_files = Vec::new();
    for entry in fs::read_dir(templates)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "jinja")
        {
            let content = fs::read_to_string(&path)?;
            if content.contains("{{ns.") || content.contains("{% for ns") {
                legacy_files.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }

    if legacy_files.is_empty() {
        Ok(())
    } else {
        legacy_files.sort();
        Err(format!(
            "S-Core template directory uses legacy per-namespace templates ({}) but the current reference codegen passes whole-model context through `json`; use the final compatible S-Core template package or the templates from the codegen checkout",
            legacy_files.join(", ")
        )
        .into())
    }
}

fn ensure_codegen_checkout(args: &CodegenArgs) -> Result<()> {
    if args.codegen_root.is_dir() {
        return Ok(());
    }

    let Some(git_url) = &args.codegen_git_url else {
        return Err(format!(
            "reference codegen checkout not found: {}\nset MW_COM_CODEGEN_ROOT, pass --codegen-root, or provide --codegen-git-url to clone it",
            args.codegen_root.display()
        )
        .into());
    };

    if let Some(parent) = args.codegen_root.parent() {
        fs::create_dir_all(parent)?;
    }

    println!(
        "Cloning reference codegen checkout from {} into {}",
        git_url,
        args.codegen_root.display()
    );
    run(Command::new("git")
        .arg("clone")
        .arg(git_url)
        .arg(&args.codegen_root))?;

    if let Some(git_ref) = &args.codegen_ref {
        run(Command::new("git")
            .current_dir(&args.codegen_root)
            .arg("checkout")
            .arg(git_ref))?;
    }

    Ok(())
}

fn ensure_file(path: &Path, label: &str) -> Result<()> {
    if path.is_file() {
        Ok(())
    } else {
        Err(format!("{label} not found: {}", path.display()).into())
    }
}

fn ensure_dir(path: &Path, label: &str) -> Result<()> {
    if path.is_dir() {
        Ok(())
    } else {
        Err(format!("{label} not found: {}", path.display()).into())
    }
}

fn run(command: &mut Command) -> Result<()> {
    let status = command
        .stdin(Stdio::null())
        .status()
        .map_err(|error| format!("failed to start command: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("command failed with status {status}").into())
    }
}

fn validate_generated_json(path: &Path) -> Result<()> {
    ensure_file(path, "generated provider config")?;
    let content = fs::read_to_string(path)?;
    serde_json::from_str::<serde_json::Value>(&content)
        .map_err(|error| format!("invalid generated JSON {}: {error}", path.display()))?;
    Ok(())
}

fn format_generated_rust(out: &Path) -> Result<()> {
    let mut rust_files = fs::read_dir(out)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect::<Vec<_>>();
    rust_files.sort();

    if rust_files.is_empty() {
        return Ok(());
    }

    let mut command = Command::new("rustfmt");
    command.arg("--edition").arg("2021");
    command.args(&rust_files);
    run(&mut command)
}

fn verify_generated_is_clean(out: &Path) -> Result<()> {
    let status = Command::new("git")
        .arg("diff")
        .arg("--exit-code")
        .arg("--")
        .arg(out)
        .status()
        .map_err(|error| format!("failed to start git diff: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err("generated artifacts changed; commit regenerated files or rerun codegen locally".into())
    }
}

fn print_usage() {
    eprintln!(
        "Usage:\n  cargo run -p xtask -- codegen [OPTIONS]\n  cargo run -p xtask -- verify-codegen [OPTIONS]\n\nOptions:\n  --model <FILE>             Input model JSON [default: models/vehicle_dynamics.json]\n  --templates <DIR>          Provider template directory [default: templates/reference-codegen]\n  --out <DIR>                Provider generated output directory [default: generated]\n  --prefix <PREFIX>          Provider output file prefix [default: mw_com]\n  --score-templates <DIR>    Optional S-Core API template directory [env: MW_COM_SCORE_TEMPLATES]\n  --score-out <DIR>          S-Core API generated output directory [default: generated/score-com-api]\n  --score-prefix <PREFIX>    S-Core API output file prefix [default: mw_com]\n  --codegen-root <DIR>       Reference codegen checkout [env: MW_COM_CODEGEN_ROOT]\n  --codegen-git-url <URL>    Clone reference codegen when checkout is missing [env: MW_COM_CODEGEN_GIT_URL]\n  --codegen-ref <REF>        Git branch, tag, or commit to checkout after clone [env: MW_COM_CODEGEN_GIT_REF]"
    );
}
