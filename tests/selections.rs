mod common;
use common::{hash, Fixture, HOST, ORIGIN};
use rust_mirror::{selection::*, Source};
use std::sync::Mutex;
fn config() -> Config {
    Config {
        schema: 1,
        rustup_version: "1.28.2".into(),
        max_download_bytes: 10 * 1024 * 1024,
        selections: vec![Selection {
            name: "dev".into(),
            toolchain: "1.90.0".into(),
            hosts: vec![HOST.into()],
            profile: "minimal".into(),
            components: vec![],
            targets: vec![],
        }],
    }
}
fn extended() -> Fixture {
    let mut f = Fixture::new();
    let mut m = Fixture::manifest();
    m=m.replace("[pkg.rust]", "default = [\"rustc\",\"cargo\",\"rust-std\",\"clippy-preview\",\"rustfmt-preview\"]\n[pkg.rust]");
    m=m.replace("components = [", &format!("extensions = [{{pkg=\"clippy-preview\",target=\"{HOST}\"}},{{pkg=\"rustfmt-preview\",target=\"{HOST}\"}},{{pkg=\"rust-src\",target=\"*\"}},{{pkg=\"rust-std\",target=\"wasm32-unknown-unknown\"}}]\ncomponents = ["));
    m.push_str("\n[renames.clippy]\nto = \"clippy-preview\"\n[renames.rustfmt]\nto = \"rustfmt-preview\"\n");
    for (pkg, target) in [
        ("clippy-preview", HOST),
        ("rustfmt-preview", HOST),
        ("rust-src", "*"),
    ] {
        m.push_str(&format!("\n[pkg.{pkg}]\nversion = \"test\"\n[pkg.{pkg}.target.'{target}']\navailable = true\nxz_url = \"{ORIGIN}dist/2025-09-18/{pkg}.tar.xz\"\nxz_hash = \"{}\"\n",hash(b"test artifact")));
        f.files.insert(
            format!("{ORIGIN}dist/2025-09-18/{pkg}.tar.xz"),
            b"test artifact".to_vec(),
        );
    }
    m.push_str(&format!("\n[pkg.rust-std.target.wasm32-unknown-unknown]\navailable = true\nxz_url = \"{ORIGIN}dist/2025-09-18/std-wasm.tar.xz\"\nxz_hash = \"{}\"\n",hash(b"test artifact")));
    f.files.insert(
        format!("{ORIGIN}dist/2025-09-18/std-wasm.tar.xz"),
        b"test artifact".to_vec(),
    );
    f.set_manifest(m);
    f
}
struct Counted {
    inner: Fixture,
    requests: Mutex<Vec<String>>,
}
impl Source for Counted {
    fn fetch(
        &self,
        url: &str,
        out: &mut dyn std::io::Write,
        limit: u64,
    ) -> rust_mirror::Result<()> {
        self.requests.lock().unwrap().push(url.into());
        self.inner.fetch(url, out, limit)
    }
}
#[test]
fn resolve_only_reads_metadata_and_records_alias_target_closure() {
    let source = Counted {
        inner: extended(),
        requests: Mutex::new(vec![]),
    };
    let mut c = config();
    c.selections[0].profile = "default".into();
    c.selections[0].components = vec!["rust-src".into(), "clippy".into()];
    c.selections[0].targets = vec!["wasm32-unknown-unknown".into()];
    let resolved = resolve(&c, "dev", &source).unwrap();
    let names: Vec<_> = resolved
        .plan
        .artifacts
        .iter()
        .map(|a| a.package.as_str())
        .collect();
    assert!(names.contains(&"clippy-preview"));
    assert!(names.contains(&"rustfmt-preview"));
    assert!(names.contains(&"rust-src"));
    assert!(resolved
        .plan
        .artifacts
        .iter()
        .any(|a| a.target == "wasm32-unknown-unknown"));
    assert!(source
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|u| u.ends_with(".toml") || u.ends_with(".sha256") || u.ends_with(".asc")));
    assert_eq!(resolved.plan.artifacts.len(), 11); // 7 components, 3 alternate encodings, bootstrap.
}
#[test]
fn frozen_plan_roundtrip_and_fetch_consult_no_live_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("plan");
    let source = extended();
    let resolved = resolve(&config(), "dev", &source).unwrap();
    resolved.save(&folder).unwrap();
    assert_eq!(load_plan(&folder).unwrap().plan, resolved.plan);
    let mut payload_only = source;
    payload_only
        .files
        .retain(|k, _| !k.ends_with(".toml") && !k.ends_with(".sha256") && !k.ends_with(".asc"));
    let mirror = fetch(&folder, &temp.path().join("output"), &payload_only).unwrap();
    let inventory: SelectionSnapshot =
        toml::from_str(&std::fs::read_to_string(mirror.join("snapshot.toml")).unwrap()).unwrap();
    assert_eq!(inventory.schema, 2);
    assert!(mirror
        .join("dist/2025-09-18/channel-rust-1.90.0.toml")
        .exists());
}
#[test]
fn supports_multiple_hosts_and_windows_bootstrap_suffix() {
    let mut source = extended();
    let raw = source.files[&format!("{ORIGIN}dist/channel-rust-1.90.0.toml")].clone();
    let mut m: toml::Value = toml::from_str(std::str::from_utf8(&raw).unwrap()).unwrap();
    let windows = "x86_64-pc-windows-msvc";
    for (_, pkg) in m["pkg"].as_table_mut().unwrap().iter_mut() {
        let targets = pkg["target"].as_table_mut().unwrap();
        if let Some(mut entry) = targets.get(HOST).cloned() {
            for kind in ["components", "extensions"] {
                if let Some(array) = entry.get_mut(kind).and_then(toml::Value::as_array_mut) {
                    for c in array {
                        if c["target"].as_str() == Some(HOST) {
                            c["target"] = toml::Value::String(windows.into());
                        }
                    }
                }
            }
            targets.insert(windows.into(), entry);
        }
    }
    source.set_manifest(toml::to_string(&m).unwrap());
    source.files.insert(
        format!("{ORIGIN}rustup/archive/1.28.2/{windows}/rustup-init.exe.sha256"),
        hash(b"bootstrap").into_bytes(),
    );
    let mut c = config();
    c.selections[0].hosts.push(windows.into());
    let p = resolve(&c, "dev", &source).unwrap();
    assert!(p
        .plan
        .artifacts
        .iter()
        .any(|a| a.path.ends_with("rustup-init.exe")));
    assert_eq!(
        p.plan
            .artifacts
            .iter()
            .filter(|a| a.kind == "bootstrap")
            .count(),
        2
    );
}
#[test]
fn catalog_reports_aliases_and_unavailable_entries() {
    let source = extended();
    let catalog = catalog("1.90.0", Some(HOST), &source).unwrap();
    assert!(catalog.hosts.iter().any(|h| h.name == HOST && h.available));
    assert!(catalog
        .components
        .iter()
        .any(|c| c.aliases.contains(&"clippy".into())));
    assert!(catalog
        .targets
        .iter()
        .any(|t| t.name == "wasm32-unknown-unknown"));
}
#[test]
fn missing_components_targets_hosts_and_profiles_never_silently_fallback() {
    for (field, value) in [
        ("component", "miri"),
        ("target", "unprovided-target"),
        ("host", "unprovided-host"),
        ("profile", "complete"),
    ] {
        let mut c = config();
        match field {
            "component" => c.selections[0].components = vec![value.into()],
            "target" => c.selections[0].targets = vec![value.into()],
            "host" => c.selections[0].hosts = vec![value.into()],
            _ => c.selections[0].profile = value.into(),
        }
        assert!(resolve(&c, "dev", &extended()).is_err(), "{field}");
    }
}
#[test]
fn tampered_and_unexpected_plan_files_refuse() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("plan");
    resolve(&config(), "dev", &extended())
        .unwrap()
        .save(&root)
        .unwrap();
    let original = std::fs::read_to_string(root.join("plan.toml")).unwrap();
    std::fs::write(
        root.join("plan.toml"),
        original.replace("cargo.tar.xz", "evil.tar.xz"),
    )
    .unwrap();
    assert!(load_plan(&root).is_err());
    std::fs::write(root.join("plan.toml"), original).unwrap();
    std::fs::write(root.join("extra"), b"extra").unwrap();
    assert!(load_plan(&root).is_err());
    std::fs::remove_file(root.join("extra")).unwrap();
    std::fs::write(root.join("manifest.asc"), b"changed").unwrap();
    assert!(load_plan(&root).is_err());
}
#[test]
fn corrupt_download_and_budget_failure_never_publish() {
    for budget in [1, 10 * 1024 * 1024] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("plan");
        let mut c = config();
        c.max_download_bytes = budget;
        let mut source = extended();
        resolve(&c, "dev", &source).unwrap().save(&root).unwrap();
        if budget > 1 {
            // A verified content-addressed file can satisfy identical variants;
            // corrupt all copies so this still exercises failed verification.
            for data in source.files.values_mut() {
                if data == b"test artifact" {
                    *data = b"corrupt".to_vec();
                }
            }
        }
        let output = temp.path().join("output");
        assert!(fetch(&root, &output, &source).is_err());
        assert!(!output.join("mirror").exists());
    }
}
#[test]
fn saved_config_strict_schema_duplicate_lists_and_dates_are_validated() {
    let example = include_str!("../examples/mirror.toml");
    assert!(Config::parse(example).is_ok());
    assert!(Config::parse(&example.replace("schema = 1", "schema = 999")).is_err());
    assert!(Config::parse(&(example.to_owned() + "\nunrecognized=true\n")).is_err());
    for toolchain in [
        "../stable",
        "nightly-2025-02-29",
        "nightly-2024-13-01",
        "nightly-2024-01-00",
        "1.90",
    ] {
        let mut c = config();
        c.selections[0].toolchain = toolchain.into();
        assert!(c.validate().is_err());
    }
    let mut c = config();
    c.selections[0].hosts.push(HOST.into());
    assert!(c.validate().is_err());
    c = config();
    c.selections[0].toolchain = "nightly-2024-02-29".into();
    assert!(c.validate().is_ok());
}
#[test]
fn imports_project_files_without_guessing_hosts_or_local_builds() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("rust-toolchain.toml");
    std::fs::write(&path,"[toolchain]\nchannel='1.90.0'\nprofile='minimal'\ncomponents=['clippy']\ntargets=['wasm32-unknown-unknown']\n").unwrap();
    let c = import_toolchain(&path, "project", vec![HOST.into()], "1.28.2").unwrap();
    assert_eq!(c.selections[0].components, vec!["clippy"]);
    assert!(import_toolchain(&path, "project", vec![], "1.28.2").is_err());
    std::fs::write(&path, "[toolchain]\npath='/local/build'\n").unwrap();
    assert!(import_toolchain(&path, "project", vec![HOST.into()], "1.28.2").is_err());
    let legacy = t.path().join("rust-toolchain");
    std::fs::write(&legacy, "1.90.0\n").unwrap();
    assert_eq!(
        import_toolchain(&legacy, "legacy", vec![HOST.into()], "1.28.2")
            .unwrap()
            .selections[0]
            .profile,
        "default"
    );
}
#[test]
fn frozen_moving_channel_has_dated_install_alias() {
    let mut source = extended();
    for ext in ["toml", "toml.sha256", "toml.asc"] {
        let data = source.files[&format!("{ORIGIN}dist/channel-rust-1.90.0.{ext}")].clone();
        source
            .files
            .insert(format!("{ORIGIN}dist/channel-rust-stable.{ext}"), data);
    }
    let mut c = config();
    c.selections[0].toolchain = "stable".into();
    let p = resolve(&c, "dev", &source).unwrap();
    assert_eq!(p.plan.pinned_toolchain, "stable-2025-09-18");
    assert_eq!(p.plan.resolved_release, "1.90.0");
}

#[test]
fn existing_outputs_refuse_without_fetching_or_overwriting() {
    let temp = tempfile::tempdir().unwrap();
    let plan = temp.path().join("plan");
    let resolved = resolve(&config(), "dev", &extended()).unwrap();
    resolved.save(&plan).unwrap();
    assert!(resolved.save(&plan).is_err());
    let output = temp.path().join("output");
    std::fs::create_dir(&output).unwrap();
    std::fs::write(output.join("keep"), b"old").unwrap();
    let source = Counted {
        inner: extended(),
        requests: Mutex::new(vec![]),
    };
    assert!(fetch(&plan, &output, &source).is_err());
    assert!(source.requests.lock().unwrap().is_empty());
    assert_eq!(std::fs::read(output.join("keep")).unwrap(), b"old");
}
#[cfg(unix)]
#[test]
fn plan_symlinks_and_fifos_are_refused_before_opening() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("plan");
    resolve(&config(), "dev", &extended())
        .unwrap()
        .save(&root)
        .unwrap();
    let link = temp.path().join("linked");
    std::os::unix::fs::symlink(&root, &link).unwrap();
    assert!(load_plan(&link).is_err());
    std::fs::rename(root.join("manifest.toml"), root.join("saved.toml")).unwrap();
    std::os::unix::fs::symlink(root.join("saved.toml"), root.join("manifest.toml")).unwrap();
    assert!(load_plan(&root).is_err());
    std::fs::remove_file(root.join("manifest.toml")).unwrap();
    assert!(std::process::Command::new("mkfifo")
        .arg(root.join("manifest.toml"))
        .status()
        .unwrap()
        .success());
    assert!(load_plan(&root).is_err());
}
