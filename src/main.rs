use rust_mirror::{prepare, HttpsSource, Options, Result};
use std::{
    collections::BTreeMap,
    io::{self, Write},
    path::PathBuf,
};
const HELP: &str = "rust-mirror fetch-toolchain --release VERSION --host x86_64-unknown-linux-gnu --rustup-version VERSION --output NEW_DIRECTORY\n\nPrepares a pinned minimal-profile snapshot for static hosting. No crates or GUI yet.";
fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    match args.next().and_then(|s| s.into_string().ok()).as_deref() {
        Some("--help" | "-h") | None => {
            writeln!(io::stdout().lock(), "{HELP}")?;
            return Ok(());
        }
        Some("fetch-toolchain") => (),
        _ => return Err(HELP.into()),
    }
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
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "rust-mirror: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
