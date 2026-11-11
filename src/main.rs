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
rust-mirror sync --config FILE --output RUN_DIRECTORY [--previous MIRROR] [--resume | --force]
rust-mirror generations --directory DIRECTORY
rust-mirror prune --config FILE --as-of YYYY-MM-DD --generation MIRROR [--generation MIRROR ...] [--dry-run | --apply]
rust-mirror prune --config FILE --as-of YYYY-MM-DD --directory DIRECTORY --next-previous MIRROR [--dry-run | --apply]
rust-mirror publish --source MIRROR --base-url HTTPS_URL [--ca-file PEM] [--allow-loopback-http]
rust-mirror rustup --snapshot MIRROR --version VERSION --output NEW_DIRECTORY [--promote CHANNEL-DATE ...] [--max-bytes BYTES]
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
    if command == "--version" || command == "-V" {
        writeln!(
            io::stdout().lock(),
            "rust-mirror {}",
            env!("CARGO_PKG_VERSION")
        )?;
        return Ok(());
    }
    if command == "assemble" {
        return assemble(args);
    }
    if command == "publish" {
        return publish_generation(args);
    }
    if command == "rustup" {
        return rustup_generation(args);
    }
    if command == "generations" {
        let key = args.next().ok_or("--directory required")?;
        if key != "--directory" {
            return Err("expected --directory".into());
        }
        let path = PathBuf::from(args.next().ok_or("directory value required")?);
        if args.next().is_some() {
            return Err("unexpected discovery option".into());
        }
        let catalog = selection::retention::discover(&path)?;
        writeln!(io::stdout().lock(), "{}", toml::to_string_pretty(&catalog)?)?;
        return Ok(());
    }
    if command == "prune" {
        return prune(args);
    }
    if command == "sync" {
        return sync(args);
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

fn publish_generation(mut args: impl Iterator<Item = std::ffi::OsString>) -> Result<()> {
    let mut values = BTreeMap::new();
    let mut local = false;
    while let Some(key) = args.next() {
        let key = key.into_string().map_err(|_| "option must be UTF-8")?;
        if key == "--allow-loopback-http" {
            if local {
                return Err("duplicate loopback option".into());
            }
            local = true;
            continue;
        }
        if !["--source", "--base-url", "--ca-file"].contains(&key.as_str()) {
            return Err("unknown publication option".into());
        }
        let value = args.next().ok_or("missing publication option value")?;
        if value.to_str().is_some_and(|v| v.starts_with("--"))
            || values.insert(key, value).is_some()
        {
            return Err("missing/duplicate publication option".into());
        }
    }
    let root = PathBuf::from(values.remove("--source").ok_or("--source required")?);
    let base_url = values
        .remove("--base-url")
        .ok_or("--base-url required")?
        .into_string()
        .map_err(|_| "URL must be UTF-8")?;
    let ca_file = values.remove("--ca-file").map(PathBuf::from);
    fn credential(key: &str) -> Result<Option<String>> {
        match std::env::var(key) {
            Ok(v) => Ok(Some(v)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(_) => Err("credentials must be UTF-8".into()),
        }
    }
    let credentials = match (credential("NEXUS_USERNAME")?, credential("NEXUS_PASSWORD")?) {
        (None, None) => None,
        (Some(u), Some(p)) => Some((u, p)),
        _ => return Err("set both NEXUS_USERNAME and NEXUS_PASSWORD".into()),
    };
    let report = rust_mirror::publish::publish(
        &root,
        &rust_mirror::publish::Options {
            base_url,
            ca_file,
            credentials,
            allow_loopback_http: local,
        },
    )?;
    writeln!(io::stdout().lock(), "{}", toml::to_string_pretty(&report)?)?;
    Ok(())
}

fn rustup_generation(mut args: impl Iterator<Item = std::ffi::OsString>) -> Result<()> {
    let mut values = BTreeMap::new();
    let mut channels = Vec::new();
    while let Some(key) = args.next() {
        let key = key.into_string().map_err(|_| "option must be UTF-8")?;
        if ![
            "--snapshot",
            "--version",
            "--output",
            "--promote",
            "--max-bytes",
        ]
        .contains(&key.as_str())
        {
            return Err("unknown rustup option".into());
        }
        let value = args.next().ok_or("missing rustup option value")?;
        if value.to_str().is_some_and(|v| v.starts_with("--")) {
            return Err("missing rustup option value".into());
        }
        if key == "--promote" {
            channels.push(value.into_string().map_err(|_| "channel must be UTF-8")?);
        } else if values.insert(key, value).is_some() {
            return Err("duplicate rustup option".into());
        }
    }
    let snapshot = PathBuf::from(values.remove("--snapshot").ok_or("--snapshot required")?);
    let output = PathBuf::from(values.remove("--output").ok_or("--output required")?);
    let version = values
        .remove("--version")
        .ok_or("--version required")?
        .into_string()
        .map_err(|_| "version must be UTF-8")?;
    let budget = values
        .remove("--max-bytes")
        .map(|v| v.into_string().map_err(|_| "budget must be UTF-8"))
        .transpose()?
        .map(|s| s.parse::<u64>())
        .transpose()?
        .unwrap_or(10 * 1024 * 1024 * 1024);
    let result = rust_mirror::selection::rustup::prepare(
        &snapshot,
        &version,
        &channels,
        &output,
        budget,
        &HttpsSource::default(),
    )?;
    writeln!(io::stdout().lock(), "{}", result.display())?;
    Ok(())
}

fn sync(mut args: impl Iterator<Item = std::ffi::OsString>) -> Result<()> {
    let mut values = BTreeMap::new();
    let mut resume = false;
    let mut force = false;
    while let Some(key) = args.next() {
        let key = key.into_string().map_err(|_| "option must be UTF-8")?;
        if key == "--force" {
            if force {
                return Err("duplicate --force".into());
            }
            force = true;
            continue;
        }
        if key == "--resume" {
            if resume {
                return Err("duplicate --resume".into());
            }
            resume = true;
            continue;
        }
        if !["--config", "--output", "--previous"].contains(&key.as_str()) {
            return Err(format!("unknown option: {key}").into());
        }
        let value = args.next().ok_or("missing option value")?;
        if value.to_str().is_some_and(|v| v.starts_with("--")) {
            return Err("missing option value".into());
        }
        if values.insert(key, value).is_some() {
            return Err("duplicate option".into());
        }
    }
    if resume && force {
        return Err("--resume and --force are mutually exclusive".into());
    }
    let config = rust_mirror::selection::Config::read(&PathBuf::from(
        values.remove("--config").ok_or("--config is required")?,
    ))?;
    let output = PathBuf::from(values.remove("--output").ok_or("--output is required")?);
    let previous = values.remove("--previous").map(PathBuf::from);
    let source = HttpsSource::default();
    let result = if force {
        rust_mirror::selection::sync::run_forced(&config, previous.as_deref(), &output, &source)
    } else {
        rust_mirror::selection::sync::run(&config, previous.as_deref(), &output, resume, &source)
    }?;
    let r = &result.report;
    let mut err = io::stderr().lock();
    let _ = writeln!(
        err,
        "SYNC {} | {} selections | {} files | {} recorded bytes",
        r.status, r.selections, r.files, r.recorded_bytes
    );
    for s in &r.frozen {
        let _ = writeln!(
            err,
            "  {}: {} -> {} ({})",
            s.name, s.requested, s.pinned_toolchain, s.release
        );
    }
    let _ = writeln!(
        err,
        "  metadata requests: {} | upstream payload bytes: {} | local payload bytes: {}",
        r.metadata_requests, r.upstream_payload_bytes, r.local_payload_bytes
    );
    writeln!(io::stdout().lock(), "{}", result.mirror.display())?;
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

fn prune(mut args: impl Iterator<Item = std::ffi::OsString>) -> Result<()> {
    let mut values = BTreeMap::new();
    let mut roots = Vec::new();
    let mut mode = None;
    while let Some(key) = args.next() {
        let key = key.into_string().map_err(|_| "option must be UTF-8")?;
        if ["--dry-run", "--apply"].contains(&key.as_str()) {
            if mode.replace(key).is_some() {
                return Err("choose --dry-run or --apply once".into());
            }
            continue;
        }
        if ![
            "--config",
            "--as-of",
            "--generation",
            "--directory",
            "--next-previous",
        ]
        .contains(&key.as_str())
        {
            return Err("unknown prune option".into());
        }
        let value = args.next().ok_or("missing prune option value")?;
        if value.to_str().is_some_and(|v| v.starts_with("--")) {
            return Err("missing prune option value".into());
        }
        if key == "--generation" {
            roots.push(PathBuf::from(value));
        } else if values.insert(key, value).is_some() {
            return Err("duplicate prune option".into());
        }
    }
    let config = rust_mirror::selection::Config::read(&PathBuf::from(
        values.remove("--config").ok_or("--config required")?,
    ))?;
    let date = values
        .remove("--as-of")
        .ok_or("--as-of required")?
        .into_string()
        .map_err(|_| "date must be UTF-8")?;
    let policy = config
        .retention
        .as_ref()
        .ok_or("pruning disabled: configure [retention] explicitly")?;
    let plan = match (
        values.remove("--directory"),
        values.remove("--next-previous"),
    ) {
        (Some(directory), Some(previous)) if roots.is_empty() => {
            rust_mirror::selection::retention::preview_directory(
                policy,
                &PathBuf::from(directory),
                &PathBuf::from(previous),
                &date,
            )?
        }
        (None, None) if !roots.is_empty() => {
            rust_mirror::selection::retention::preview(policy, &roots, &date)?
        }
        _ => {
            return Err("use either --generation roots or --directory with --next-previous".into())
        }
    };
    if mode.as_deref() == Some("--apply") {
        let report = rust_mirror::selection::retention::apply(&plan, true)?;
        writeln!(io::stdout().lock(), "{}", toml::to_string_pretty(&report)?)?;
        if report.failure.is_some() {
            return Err(
                "pruning stopped after a partial deletion; inspect the structured report".into(),
            );
        }
    } else {
        writeln!(io::stdout().lock(), "{}", toml::to_string_pretty(&plan)?)?;
    }
    Ok(())
}
