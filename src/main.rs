use rust_mirror::{prepare, HttpsSource, Options, Result};
use std::{
    collections::BTreeMap,
    io::{self, Write},
    path::PathBuf,
};
const HELP: &str = "rust-mirror init --config FILE
rust-mirror list --toolchain TOOLCHAIN [--host HOST]
rust-mirror import --toolchain-file FILE --name NAME --hosts HOST[,HOST] --rustup-version VERSION --output FILE
rust-mirror plan --config FILE --selection NAME --output NEW_PLAN_DIRECTORY
rust-mirror fetch --plan PLAN_DIRECTORY --output SNAPSHOT_DIRECTORY [--resume]
rust-mirror assemble --snapshot MIRROR [--snapshot MIRROR ...] [--previous MIRROR] --output NEW_DIRECTORY [--max-bytes BYTES]
rust-mirror fetch-toolchain --release VERSION --host x86_64-unknown-linux-gnu --rustup-version VERSION --output NEW_DIRECTORY

Choose versions, hosts, components and targets in saved TOML selections. Plan reads metadata only; fetch replays a frozen plan. The legacy fetch-toolchain route remains minimal Linux only. No crates or GUI yet.";
fn legacy(args: impl Iterator<Item = std::ffi::OsString>) -> Result<()> {
    let mut args = args;
    let mut values = BTreeMap::new();
    while let Some(key) = args.next() {
        let key = key.into_string().map_err(|_| "option name must be UTF-8")?;
        if !["--release", "--host", "--rustup-version", "--output"].contains(&key.as_str()) {
            return Err(format!("unknown option: {key}").into());
        }
        let value = args.next().ok_or("missing option value")?;
        if values.insert(key, value).is_some() {
            return Err("duplicate option".into());
        }
    }
    let output = PathBuf::from(values.remove("--output").ok_or("--output is required")?);
    let mut text = |key: &str| -> Result<String> {
        Ok(values
            .remove(key)
            .ok_or(format!("{key} is required"))?
            .into_string()
            .map_err(|_| "version/host must be UTF-8")?)
    };
    let options = Options {
        release: text("--release")?,
        host: text("--host")?,
        rustup_version: text("--rustup-version")?,
        output,
    };
    let path = prepare(&options, &HttpsSource::default())?;
    writeln!(io::stdout().lock(), "{}", path.display())?;
    Ok(())
}

fn new_output(path: &std::path::Path) -> Result<()> {
    if path.try_exists()? {
        return Err("output already exists; select a new path".into());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    if !parent.is_dir() {
        return Err("output parent directory must exist".into());
    }
    Ok(())
}
fn run() -> Result<()> {
    use rust_mirror::selection;
    let mut args = std::env::args_os().skip(1);
    let command = args
        .next()
        .unwrap_or_else(|| "--help".into())
        .into_string()
        .map_err(|_| "command must be UTF-8")?;
    if command == "--help" || command == "-h" {
        writeln!(io::stdout().lock(), "{HELP}")?;
        return Ok(());
    }
    if command == "fetch-toolchain" {
        return legacy(args);
    }
    if command == "assemble" {
        return assemble(args);
    }
    let allowed: &[&str] = match command.as_str() {
        "init" => &["--config"],
        "list" => &["--toolchain", "--host"],
        "import" => &[
            "--toolchain-file",
            "--name",
            "--hosts",
            "--rustup-version",
            "--output",
        ],
        "plan" => &["--config", "--selection", "--output"],
        "fetch" => &["--plan", "--output"],
        _ => return Err(HELP.into()),
    };
    let mut resume = false;
    let mut values = BTreeMap::new();
    while let Some(key) = args.next() {
        let key = key.into_string().map_err(|_| "option name must be UTF-8")?;
        if command == "fetch" && key == "--resume" {
            if resume {
                return Err("duplicate --resume".into());
            }
            resume = true;
            continue;
        }
        if !allowed.contains(&key.as_str()) {
            return Err(format!("unknown option: {key}").into());
        }
        let value = args.next().ok_or("missing option value")?;
        if value.to_str().is_some_and(|v| v.starts_with("--")) {
            return Err(format!("missing value for {key}").into());
        }
        if values.insert(key, value).is_some() {
            return Err("duplicate option".into());
        }
    }
    let path = |key: &str| -> Result<PathBuf> {
        Ok(PathBuf::from(
            values.get(key).ok_or(format!("{key} is required"))?,
        ))
    };
    let text = |key: &str| -> Result<String> {
        values
            .get(key)
            .ok_or_else(|| format!("{key} is required").into())
            .and_then(|v| {
                v.to_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("{key} must be UTF-8").into())
            })
    };
    let source = HttpsSource::default();
    match command.as_str() {
        "init" => {
            let output = path("--config")?;
            new_output(&output)?;
            let config = selection::Config::parse(include_str!("../examples/mirror.toml"))?;
            selection::write_config(&config, &output)?;
            writeln!(io::stdout().lock(), "{}", output.display())?;
        }
        "list" => {
            let toolchain = text("--toolchain")?;
            let host = values
                .get("--host")
                .map(|v| v.to_str().ok_or("host must be UTF-8"))
                .transpose()?;
            let data = selection::catalog(&toolchain, host, &source)?;
            writeln!(io::stdout().lock(), "{}", toml::to_string_pretty(&data)?)?;
        }
        "import" => {
            let output = path("--output")?;
            new_output(&output)?;
            let config = selection::import_toolchain(
                &path("--toolchain-file")?,
                &text("--name")?,
                text("--hosts")?.split(',').map(str::to_owned).collect(),
                &text("--rustup-version")?,
            )?;
            selection::write_config(&config, &output)?;
            writeln!(io::stdout().lock(), "{}", output.display())?;
        }
        "plan" => {
            let output = path("--output")?;
            new_output(&output)?;
            let config = selection::Config::read(&path("--config")?)?;
            let resolved = selection::resolve(&config, &text("--selection")?, &source)?;
            resolved.save(&output)?;
            let _=writeln!(io::stderr().lock(),"{} artifacts; archive sizes unknown until download; budget {} bytes. Saved plan: {}",resolved.plan.artifacts.len(),resolved.plan.max_download_bytes,output.display());
            writeln!(
                io::stdout().lock(),
                "{}",
                toml::to_string_pretty(&resolved.plan)?
            )?;
        }
        "fetch" => {
            let output = path("--output")?;
            if !resume {
                new_output(&output)?;
            }
            let completed = if resume {
                selection::fetch_resuming(&path("--plan")?, &output, &source)
            } else {
                selection::fetch(&path("--plan")?, &output, &source)
            }
            .map_err(|e| {
                format!(
                    "{e}; if this output was initialized, retry the same plan/output with --resume"
                )
            })?;
            writeln!(io::stdout().lock(), "{}", completed.display())?;
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn assemble(mut args: impl Iterator<Item = std::ffi::OsString>) -> Result<()> {
    let mut inputs = Vec::new();
    let mut values = BTreeMap::new();
    while let Some(key) = args.next() {
        let key = key.into_string().map_err(|_| "option must be UTF-8")?;
        if !["--snapshot", "--previous", "--output", "--max-bytes"].contains(&key.as_str()) {
            return Err(format!("unknown option: {key}").into());
        }
        let value = args.next().ok_or("missing option value")?;
        if value.to_str().is_some_and(|v| v.starts_with("--")) {
            return Err("missing option value".into());
        }
        if key == "--snapshot" {
            inputs.push(PathBuf::from(value));
        } else if values.insert(key, value).is_some() {
            return Err("duplicate option".into());
        }
    }
    let output = PathBuf::from(values.remove("--output").ok_or("--output is required")?);
    new_output(&output)?;
    let previous = values.remove("--previous").map(PathBuf::from);
    let budget = values
        .remove("--max-bytes")
        .map(|v| -> Result<u64> { Ok(v.to_str().ok_or("byte count must be UTF-8")?.parse()?) })
        .transpose()?
        .unwrap_or(10 * 1024 * 1024 * 1024);
    let path = rust_mirror::selection::collection::assemble(
        &inputs,
        previous.as_deref(),
        &output,
        budget,
    )?;
    writeln!(io::stdout().lock(), "{}", path.display())?;
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "rust-mirror: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
