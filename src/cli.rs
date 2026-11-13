//! Shared human/event CLI adapter. The standalone binary installs signal handling;
//! library embeddings supply their own cancellation token and process policy.
mod crates;
mod network_options;
use crate::events::{CancelToken, Control, EventKind, JsonLines, PROTOCOL_VERSION};
use std::{
    cell::RefCell,
    io::{BufRead, Read},
    sync::Arc,
};
thread_local! {static CAPTURE:RefCell<Option<String>>=const{RefCell::new(None)};static REPORT:RefCell<Option<toml::Value>>=const{RefCell::new(None)};}
fn capturing() -> bool {
    CAPTURE.with(|c| c.borrow().is_some())
}
fn emit_output(text: String) -> Result<()> {
    if capturing() {
        CAPTURE.with(|c| {
            let mut c = c.borrow_mut();
            c.as_mut().unwrap().push_str(&text);
        });
    } else {
        writeln!(io::stdout().lock(), "{text}")?;
    }
    Ok(())
}
macro_rules! output {($($args:tt)*)=>{emit_output(format!($($args)*))};}
use crate::{prepare, HttpsSource, Options, Result};
use std::{
    collections::BTreeMap,
    io::{self, Write},
    path::PathBuf,
};
const HELP: &str = "rust-mirror [--events jsonl] [--cancel-stdin] COMMAND ...
rust-mirror check-network [NETWORK_FLAGS]
rust-mirror releases --cache DIRECTORY [--refresh | --offline] [NETWORK_FLAGS]
rust-mirror crates --help
rust-mirror init --config FILE
rust-mirror list --toolchain TOOLCHAIN [--host HOST]
rust-mirror import --toolchain-file FILE --name NAME --hosts HOST[,HOST] --rustup-version VERSION --output FILE
rust-mirror plan --config FILE --selection NAME --output NEW_PLAN_DIRECTORY
rust-mirror fetch --plan PLAN_DIRECTORY --output SNAPSHOT_DIRECTORY [--resume]
rust-mirror sync --config FILE --output RUN_DIRECTORY [--previous MIRROR] [--resume | --force]
rust-mirror generations --directory DIRECTORY
rust-mirror prune --config FILE --as-of YYYY-MM-DD --generation MIRROR [--generation MIRROR ...] [--dry-run | --apply]
rust-mirror prune --config FILE --as-of YYYY-MM-DD --directory DIRECTORY --next-previous MIRROR [--dry-run | --apply]
rust-mirror publish --source MIRROR --base-url HTTPS_URL [--preflight] [--ca-file PEM] [--allow-loopback-http]
rust-mirror rustup --snapshot MIRROR --version VERSION --output NEW_DIRECTORY [--promote CHANNEL-DATE ...] [--max-bytes BYTES]
rust-mirror assemble --snapshot MIRROR [--snapshot MIRROR ...] [--previous MIRROR] --output NEW_DIRECTORY [--max-bytes BYTES]
rust-mirror fetch-toolchain --release VERSION --host x86_64-unknown-linux-gnu --rustup-version VERSION --output NEW_DIRECTORY

Choose versions, hosts, components and targets in saved TOML selections. Plan reads metadata only; fetch replays a frozen plan. The legacy fetch-toolchain route remains minimal Linux only. Crate registry commands are experimental development-source functionality; see crates --help. Upstream network flags: --proxy URL | --proxy-from-env, --no-proxy LIST, --proxy-username-env NAME, --proxy-password-env NAME | --proxy-password-stdin, --extra-ca-file PEM, --connect-timeout SECONDS, --read-timeout SECONDS, --network-config FILE. Plan/sync accept --estimate-sizes.";
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
    let path = prepare(&options, &network_options::source(None)?)?;
    output!("{}", path.display())?;
    Ok(())
}

fn new_output(path: &std::path::Path) -> Result<()> {
    if path.try_exists()? {
        return Err(crate::Error::new(
            crate::ErrorKind::OutputExists,
            "output already exists; select a new path",
        )
        .into());
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
fn run(args: impl Iterator<Item = std::ffi::OsString>) -> Result<()> {
    let (args, _reset) = network_options::prepare(args)?;
    run_prepared(args.into_iter())
}
fn run_prepared(mut args: impl Iterator<Item = std::ffi::OsString>) -> Result<()> {
    use crate::selection;
    let command = args
        .next()
        .unwrap_or_else(|| "--help".into())
        .into_string()
        .map_err(|_| "command must be UTF-8")?;
    if command == "crates" {
        return self::crates::run(args);
    }
    if command == "--help" || command == "-h" {
        output!("{HELP}")?;
        return Ok(());
    }
    if command == "fetch-toolchain" {
        return legacy(args);
    }
    if command == "--version" || command == "-V" {
        output!("rust-mirror {}", env!("CARGO_PKG_VERSION"))?;
        return Ok(());
    }
    if command == "check-network" {
        if args.next().is_some() {
            return Err("check-network accepts only global network flags".into());
        }
        let report = crate::network::check_network_with_control(
            &network_options::options(None)?,
            &crate::events::current().unwrap_or_default(),
        )?;
        if !report.ok {
            let kind = report
                .steps
                .iter()
                .find_map(|s| s.error_kind)
                .unwrap_or(crate::ErrorKind::Network);
            let mut error = crate::Error::new(
                kind,
                "connection check failed; inspect the ordered diagnostic report",
            );
            error.report = Some(toml::Value::try_from(&report)?);
            return Err(error.into());
        }
        output!("{}", toml::to_string_pretty(&report)?)?;
        return Ok(());
    }
    if command == "releases" {
        let mut directory = None;
        let mut mode = crate::releases::CacheMode::RefreshOrCached;
        let mut chosen = false;
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--cache") => {
                    if directory.is_some() {
                        return Err("duplicate --cache".into());
                    }
                    directory = Some(PathBuf::from(
                        args.next().ok_or("cache directory required")?,
                    ));
                }
                Some("--refresh" | "--offline") => {
                    if chosen {
                        return Err("choose --refresh or --offline once".into());
                    }
                    chosen = true;
                    mode = if arg == "--offline" {
                        crate::releases::CacheMode::Offline
                    } else {
                        crate::releases::CacheMode::Refresh
                    };
                }
                _ => return Err("unknown releases option".into()),
            }
        }
        let report = crate::releases::load(
            &network_options::source(None)?,
            &directory.ok_or("--cache required")?,
            mode,
            &crate::events::current().unwrap_or_default(),
        )?;
        output!("{}", toml::to_string_pretty(&report)?)?;
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
        output!("{}", toml::to_string_pretty(&catalog)?)?;
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
    let source = network_options::source(None)?;
    match command.as_str() {
        "init" => {
            let output = path("--config")?;
            new_output(&output)?;
            let config = selection::Config::parse(include_str!("../examples/mirror.toml"))?;
            selection::write_config(&config, &output)?;
            output!("{}", output.display())?;
        }
        "list" => {
            let toolchain = text("--toolchain")?;
            let host = values
                .get("--host")
                .map(|v| v.to_str().ok_or("host must be UTF-8"))
                .transpose()?;
            let data = selection::catalog(&toolchain, host, &source)?;
            output!("{}", toml::to_string_pretty(&data)?)?;
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
            output!("{}", output.display())?;
        }
        "plan" => {
            let output = path("--output")?;
            new_output(&output)?;
            let config = selection::Config::read(&path("--config")?)?;
            let source = network_options::source(config.network.as_ref())?;
            let resolved = selection::resolve(&config, &text("--selection")?, &source)?;
            let sizes = if network_options::estimate() {
                Some(selection::measure_sizes(
                    &resolved,
                    &source,
                    &crate::events::current().unwrap_or_default(),
                )?)
            } else {
                None
            };
            resolved.save(&output)?;
            if let Some(sizes) = sizes {
                let mut value = toml::map::Map::new();
                value.insert("plan".into(), toml::Value::try_from(&resolved.plan)?);
                value.insert("sizes".into(), toml::Value::try_from(&sizes)?);
                if capturing() {
                    REPORT.with(|r| *r.borrow_mut() = Some(toml::Value::Table(value.clone())));
                }
                output!("{}", toml::to_string_pretty(&value)?)?;
                let _ = writeln!(
                    io::stderr().lock(),
                    "Estimated known bytes: {}; unknown artifacts: {}; exceeds budget: {}",
                    sizes.known_bytes,
                    sizes.unknown_artifacts,
                    sizes.exceeds_budget
                );
            }
            if !network_options::estimate() {
                let _=writeln!(io::stderr().lock(),"{} artifacts; archive sizes unknown until download; budget {} bytes. Saved plan: {}",resolved.plan.artifacts.len(),resolved.plan.max_download_bytes,output.display());
                output!("{}", toml::to_string_pretty(&resolved.plan)?)?;
            }
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
                crate::Error::new(crate::error_kind(e.as_ref()), format!("{e}; if this output was initialized, retry the same plan/output with --resume"))
            })?;
            output!("{}", completed.display())?;
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
    let path =
        crate::selection::collection::assemble(&inputs, previous.as_deref(), &output, budget)?;
    output!("{}", path.display())?;
    Ok(())
}

fn publish_generation(mut args: impl Iterator<Item = std::ffi::OsString>) -> Result<()> {
    let mut values = BTreeMap::new();
    let mut local = false;
    let mut check_only = false;
    while let Some(key) = args.next() {
        let key = key.into_string().map_err(|_| "option must be UTF-8")?;
        if key == "--preflight" {
            if check_only {
                return Err("duplicate --preflight".into());
            }
            check_only = true;
            continue;
        }
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
    let options = crate::publish::Options {
        base_url,
        ca_file,
        credentials,
        allow_loopback_http: local,
    };
    if check_only {
        let report = if network_options::explicit() {
            crate::publish::preflight_with_network(
                &root,
                &options,
                &network_options::options(None)?,
                &crate::events::current().unwrap_or_default(),
            )?
        } else {
            crate::publish::preflight(&root, &options)?
        };
        output!("{}", toml::to_string_pretty(&report)?)?;
    } else {
        let report = if network_options::explicit() {
            crate::publish::publish_with_network(
                &root,
                &options,
                &network_options::options(None)?,
                &crate::events::current().unwrap_or_default(),
            )?
        } else {
            crate::publish::publish(&root, &options)?
        };
        output!("{}", toml::to_string_pretty(&report)?)?;
    }
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
    let result = crate::selection::rustup::prepare(
        &snapshot,
        &version,
        &channels,
        &output,
        budget,
        &network_options::source(None)?,
    )?;
    output!("{}", result.display())?;
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
    let config = crate::selection::Config::read(&PathBuf::from(
        values.remove("--config").ok_or("--config is required")?,
    ))?;
    let output = PathBuf::from(values.remove("--output").ok_or("--output is required")?);
    let previous = values.remove("--previous").map(PathBuf::from);
    let source = network_options::source(config.network.as_ref())?;
    let result = crate::selection::sync::run_with_options(
        &config,
        previous.as_deref(),
        &output,
        &crate::selection::sync::RunOptions {
            force,
            resume,
            estimate_sizes: network_options::estimate(),
        },
        &source,
        &crate::events::current().unwrap_or_default(),
    )?;
    if capturing() {
        let mut report = toml::map::Map::new();
        report.insert(
            "mirror".into(),
            toml::Value::String(result.mirror.to_string_lossy().into()),
        );
        report.insert("report".into(), toml::Value::try_from(&result.report)?);
        REPORT.with(|r| *r.borrow_mut() = Some(toml::Value::Table(report)));
    }
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
    output!("{}", result.mirror.display())?;
    Ok(())
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
    let config = crate::selection::Config::read(&PathBuf::from(
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
            crate::selection::retention::preview_directory(
                policy,
                &PathBuf::from(directory),
                &PathBuf::from(previous),
                &date,
            )?
        }
        (None, None) if !roots.is_empty() => {
            crate::selection::retention::preview(policy, &roots, &date)?
        }
        _ => {
            return Err("use either --generation roots or --directory with --next-previous".into())
        }
    };
    if mode.as_deref() == Some("--apply") {
        let report = crate::selection::retention::apply(&plan, true)?;
        output!("{}", toml::to_string_pretty(&report)?)?;
        if report.failure.is_some() {
            let mut error = crate::Error::new(
                crate::ErrorKind::Io,
                "pruning stopped after a partial deletion; inspect the structured report",
            );
            error.report = Some(toml::Value::try_from(&report)?);
            return Err(error.into());
        }
    } else {
        output!("{}", toml::to_string_pretty(&plan)?)?;
    }
    Ok(())
}

fn report(command: &str, text: &str) -> Result<toml::Value> {
    if let Some(report) = REPORT.with(|r| r.borrow_mut().take()) {
        return Ok(report);
    }
    let mut object = toml::map::Map::new();
    match command {
        "--version" | "-V" => {
            object.insert("tool_version".into(), env!("CARGO_PKG_VERSION").into());
            object.insert("protocol".into(), (PROTOCOL_VERSION as i64).into());
        }
        "--help" | "-h" => {
            object.insert("text".into(), text.into());
        }
        "init" | "import" => {
            object.insert("path".into(), text.trim_end().into());
        }
        "sync" | "fetch" | "assemble" | "rustup" | "fetch-toolchain" => {
            object.insert("mirror".into(), text.trim_end().into());
        }
        _ => return Ok(toml::from_str(text)?),
    }
    Ok(toml::Value::Table(object))
}
/// Run a command through protocol 1, sharing command dispatch with the human CLI.
/// `args` omits the executable/global events flag; `--cancel-stdin` is supported.
/// The writer is flushed per event. Returns 0, 1 or 130; killed/broken streams may
/// lack a result. No signal handlers are installed by this library function.
pub fn run_with_events<W: Write + Send + 'static>(
    args: Vec<std::ffi::OsString>,
    writer: W,
    cancel: CancelToken,
) -> i32 {
    let control = Control::new(Arc::new(JsonLines::new(writer)), cancel.clone());
    let command = network_options::command(&args);
    if control
        .emit(EventKind::Hello {
            protocol: PROTOCOL_VERSION,
            tool_version: env!("CARGO_PKG_VERSION").into(),
            command: command.clone(),
        })
        .is_err()
    {
        return 1;
    }
    let mut input = false;
    let mut clean = Vec::new();
    let mut invalid = false;
    for arg in args {
        if arg == "--cancel-stdin" {
            if input {
                invalid = true;
            }
            input = true;
        } else {
            clean.push(arg);
        }
    }
    CAPTURE.with(|c| *c.borrow_mut() = Some(String::new()));
    REPORT.with(|r| *r.borrow_mut() = None);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        control.run(&command, || {
            if invalid {
                return Err(crate::Error::new(
                    crate::ErrorKind::Invalid,
                    "duplicate --cancel-stdin",
                )
                .into());
            }
            let (prepared, _reset) = network_options::prepare(clean.into_iter())?;
            if input {
                std::thread::spawn(move || {
                    let input = std::io::stdin();
                    let mut reader = input.lock();
                    loop {
                        let mut line = String::new();
                        match reader.by_ref().take(1025).read_line(&mut line) {
                            Ok(0) => {
                                cancel.cancel();
                                break;
                            }
                            Ok(_) => {
                                let mut text = line.trim();
                                let tokens = ["{", "\"type\"", ":", "\"cancel\"", "}"];
                                let mut valid = true;
                                for token in tokens {
                                    if let Some(rest) = text.trim_start().strip_prefix(token) {
                                        text = rest;
                                    } else {
                                        valid = false;
                                        break;
                                    }
                                }
                                if valid && text.trim().is_empty() {
                                    cancel.cancel();
                                    break;
                                }
                            }
                            Err(_) => {
                                cancel.cancel();
                                break;
                            }
                        }
                    }
                });
            }
            run_prepared(prepared.into_iter())?;
            let text = CAPTURE.with(|c| c.borrow().clone().unwrap_or_default());
            report(&command, &text)
        })
    }));
    CAPTURE.with(|c| *c.borrow_mut() = None);
    REPORT.with(|r| *r.borrow_mut() = None);
    let result = match result {
        Ok(result) => result,
        Err(_) => {
            Err(crate::Error::new(crate::ErrorKind::Internal, "internal operation failure").into())
        }
    };
    let (event, code) = match result {
        Ok(report) => (
            EventKind::Result {
                ok: true,
                report: Some(report),
                error: None,
            },
            0,
        ),
        Err(error) => {
            let error = error
                .downcast_ref::<crate::Error>()
                .cloned()
                .unwrap_or_else(|| {
                    crate::Error::new(crate::error_kind(error.as_ref()), error.to_string())
                });
            let code = if error.kind == crate::ErrorKind::Cancelled {
                130
            } else {
                1
            };
            (
                EventKind::Result {
                    ok: false,
                    report: None,
                    error: Some(error),
                },
                code,
            )
        }
    };
    if control.emit(event).is_err() {
        1
    } else {
        code
    }
}
/// Execute argv from the standalone binary, preserving human mode unless events are requested.
/// Returns a process exit code; signal installation belongs to the caller.
pub fn execute(args: Vec<std::ffi::OsString>, cancel: CancelToken) -> i32 {
    let mut clean = Vec::new();
    let mut args = args.into_iter();
    let mut events = false;
    let mut invalid = None;
    while let Some(arg) = args.next() {
        if arg == "--events" {
            if events {
                invalid = Some("duplicate --events");
            }
            events = true;
            if args.next().as_deref() != Some(std::ffi::OsStr::new("jsonl")) {
                invalid = Some("--events requires jsonl");
            }
        } else {
            clean.push(arg);
        }
    }
    if events {
        if let Some(message) = invalid {
            clean = vec!["invalid-global-options".into(), message.into()];
        }
        return run_with_events(clean, io::stdout(), cancel);
    }
    if clean.iter().any(|s| s == "--cancel-stdin") {
        let _ = writeln!(io::stderr(), "--cancel-stdin requires --events jsonl");
        return 1;
    }
    let control = Control::new(Arc::new(Human), cancel);
    match control.run("command", || run(clean.into_iter())) {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(io::stderr(), "rust-mirror: {error}");
            if crate::error_kind(error.as_ref()) == crate::ErrorKind::Cancelled {
                130
            } else {
                1
            }
        }
    }
}
struct Human;
impl crate::events::Observer for Human {
    fn observe(&self, _: &crate::events::Event) -> io::Result<()> {
        Ok(())
    }
}
