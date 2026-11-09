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
