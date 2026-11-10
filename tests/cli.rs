use std::process::Command;
#[test]
fn help_is_offline_and_describes_current_scope() {
    let result = Command::new(env!("CARGO_BIN_EXE_rust-mirror"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(String::from_utf8(result.stdout)
        .unwrap()
        .contains("fetch-toolchain"));
}
#[test]
fn duplicate_and_unknown_options_refuse_before_network() {
    for args in [
        vec![
            "fetch-toolchain",
            "--release",
            "1.90.0",
            "--release",
            "1.91.0",
        ],
        vec!["fetch-toolchain", "--unknown", "value"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_rust-mirror"))
            .args(args)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn saved_selection_commands_work_offline_and_preserve_existing_files() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("mirror.toml");
    let cli = env!("CARGO_BIN_EXE_rust-mirror");
    let first = Command::new(cli)
        .arg("init")
        .arg("--config")
        .arg(&config)
        .output()
        .unwrap();
    assert!(first.status.success());
    let old = std::fs::read(&config).unwrap();
    assert!(rust_mirror::selection::Config::parse(std::str::from_utf8(&old).unwrap()).is_ok());
    assert!(!Command::new(cli)
        .arg("init")
        .arg("--config")
        .arg(&config)
        .status()
        .unwrap()
        .success());
    assert_eq!(old, std::fs::read(&config).unwrap());
    let project = temp.path().join("rust-toolchain.toml");
    std::fs::write(
        &project,
        "[toolchain]\nchannel='1.90.0'\ncomponents=['rust-src']\n",
    )
    .unwrap();
    let imported = temp.path().join("imported.toml");
    let result = Command::new(cli)
        .arg("import")
        .arg("--toolchain-file")
        .arg(&project)
        .args([
            "--name",
            "project",
            "--hosts",
            "x86_64-unknown-linux-gnu",
            "--rustup-version",
            "1.28.2",
        ])
        .arg("--output")
        .arg(&imported)
        .output()
        .unwrap();
    assert!(result.status.success(), "{:?}", result);
    assert_eq!(
        rust_mirror::selection::Config::read(&imported)
            .unwrap()
            .selections[0]
            .components,
        vec!["rust-src"]
    );
    for args in [
        vec!["plan", "--config", "--selection", "x"],
        vec!["list", "--toolchain", "stable", "--toolchain", "beta"],
    ] {
        let result = Command::new(cli).args(args).output().unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
    }
}
