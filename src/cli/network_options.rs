use super::*;
use crate::network::{NetworkConfig, NetworkOptions, ProxyCredentials};
#[derive(Default)]
struct Overrides {
    flags: BTreeMap<String, std::ffi::OsString>,
    password: Option<String>,
    estimate: bool,
}
thread_local! {static OPTIONS:RefCell<Overrides>=RefCell::new(Overrides::default());}
pub(super) struct Reset;
impl Drop for Reset {
    fn drop(&mut self) {
        OPTIONS.with(|o| *o.borrow_mut() = Overrides::default());
    }
}
pub(super) fn prepare(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(Vec<std::ffi::OsString>, Reset)> {
    let mut args = args;
    let mut clean = vec![];
    let mut value = Overrides::default();
    let mut stdin_password = false;
    while let Some(arg) = args.next() {
        let key = arg.to_str().unwrap_or("");
        if [
            "--proxy-from-env",
            "--proxy-password-stdin",
            "--estimate-sizes",
        ]
        .contains(&key)
        {
            if value.flags.insert(key.into(), "true".into()).is_some() {
                return Err("duplicate network/estimate flag".into());
            }
            if key == "--proxy-password-stdin" {
                stdin_password = true;
            }
            if key == "--estimate-sizes" {
                value.estimate = true;
            }
        } else if [
            "--proxy",
            "--no-proxy",
            "--proxy-username-env",
            "--proxy-password-env",
            "--extra-ca-file",
            "--connect-timeout",
            "--read-timeout",
            "--network-config",
        ]
        .contains(&key)
        {
            let v = args.next().ok_or("missing network option value")?;
            if v.to_str().is_some_and(|v| v.starts_with("--"))
                || value.flags.insert(key.into(), v).is_some()
            {
                return Err("missing or duplicate network option value".into());
            }
        } else if key.starts_with("--proxy") {
            return Err("unsupported proxy option syntax; use separate flag/value arguments and named-secret or stdin credential inputs".into());
        } else {
            clean.push(arg);
        }
    }
    let command = clean.first().and_then(|v| v.to_str()).unwrap_or("");
    if value.estimate && !["plan", "sync"].contains(&command) {
        return Err("--estimate-sizes is only supported by plan and sync".into());
    }
    if value.flags.contains_key("--proxy") && value.flags.contains_key("--proxy-from-env") {
        return Err("manual and environment proxy modes are mutually exclusive".into());
    }
    if !value.flags.is_empty()
        && ![
            "plan",
            "sync",
            "list",
            "fetch",
            "rustup",
            "fetch-toolchain",
            "publish",
            "check-network",
            "releases",
        ]
        .contains(&command)
    {
        return Err("network flags require a network-capable command".into());
    }
    if stdin_password {
        if value.flags.contains_key("--proxy-password-env") {
            return Err("choose password stdin or variable, not both".into());
        }
        let mut password = String::new();
        io::stdin().lock().take(4098).read_line(&mut password)?;
        if !password.ends_with('\n') || password.len() > 4097 {
            return Err("proxy password must be one bounded UTF-8 line".into());
        }
        password.pop();
        if password.ends_with('\r') {
            password.pop();
        }
        value.password = Some(password);
    }
    OPTIONS.with(|o| *o.borrow_mut() = value);
    Ok((clean, Reset))
}
pub(super) fn estimate() -> bool {
    OPTIONS.with(|o| o.borrow().estimate)
}
pub(super) fn explicit() -> bool {
    OPTIONS.with(|o| !o.borrow().flags.is_empty())
}
pub(super) fn options(base: Option<&NetworkConfig>) -> Result<NetworkOptions> {
    OPTIONS.with(|o| {
        let o = o.borrow();
        let mut config = base.cloned().unwrap_or_default();
        if let Some(path) = o.flags.get("--network-config") {
            config = crate::selection::Config::read(&PathBuf::from(path))?
                .network
                .unwrap_or_default();
        }
        let text = |key: &str| -> Result<Option<String>> {
            o.flags
                .get(key)
                .map(|v| {
                    v.to_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "network option must be UTF-8".into())
                })
                .transpose()
        };
        if let Some(url) = text("--proxy")? {
            config.proxy_mode = "manual".into();
            config.proxy_url = Some(url);
        }
        if o.flags.contains_key("--proxy-from-env") {
            config.proxy_mode = "environment".into();
            config.proxy_url = None;
        }
        if let Some(list) = text("--no-proxy")? {
            config.no_proxy = list
                .split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
                .collect();
        }
        if let Some(v) = text("--proxy-username-env")? {
            config.proxy_username_env = Some(v);
        }
        if let Some(v) = text("--proxy-password-env")? {
            config.proxy_password_env = Some(v);
        }
        if let Some(v) = o.flags.get("--extra-ca-file") {
            config.extra_ca_file = Some(PathBuf::from(v));
        }
        if let Some(v) = text("--connect-timeout")? {
            config.connect_timeout_seconds = v
                .parse()
                .map_err(|_| "connect timeout must be integer seconds")?;
        }
        if let Some(v) = text("--read-timeout")? {
            config.read_timeout_seconds = v
                .parse()
                .map_err(|_| "read timeout must be integer seconds")?;
        }
        if let Some(password) = &o.password {
            if config.proxy_password_env.is_some() {
                return Err("stdin and configured password variable conflict".into());
            }
            let user = config.proxy_username_env.take().ok_or(
                "password stdin requires --proxy-username-env or configured username variable",
            )?;
            // Validate the variable name using the same config rules without reading a placeholder.
            let mut check = config.clone();
            check.proxy_username_env = Some(user.clone());
            check.proxy_password_env = Some(user.clone());
            check.validate()?;
            let mut options = config.resolve()?;
            options.proxy_credentials = Some(ProxyCredentials {
                username: std::env::var(user)
                    .map_err(|_| "proxy username variable is missing or not UTF-8")?,
                password: password.clone(),
            });
            Ok(options)
        } else {
            config.resolve()
        }
    })
}
pub(super) fn source(base: Option<&NetworkConfig>) -> Result<HttpsSource> {
    HttpsSource::with_network(&options(base)?)
}

pub(super) fn command(args: &[std::ffi::OsString]) -> String {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some(
                "--proxy"
                | "--no-proxy"
                | "--proxy-username-env"
                | "--proxy-password-env"
                | "--extra-ca-file"
                | "--connect-timeout"
                | "--read-timeout"
                | "--network-config",
            ) => {
                args.next();
            }
            Some(
                "--proxy-from-env"
                | "--proxy-password-stdin"
                | "--estimate-sizes"
                | "--cancel-stdin",
            ) => {}
            Some(name)
                if [
                    "--help",
                    "-h",
                    "--version",
                    "-V",
                    "init",
                    "list",
                    "import",
                    "plan",
                    "fetch",
                    "sync",
                    "assemble",
                    "rustup",
                    "publish",
                    "generations",
                    "prune",
                    "fetch-toolchain",
                    "check-network",
                    "releases",
                ]
                .contains(&name) =>
            {
                return name.to_owned()
            }
            _ => return "invalid-command".into(),
        }
    }
    "--help".into()
}
