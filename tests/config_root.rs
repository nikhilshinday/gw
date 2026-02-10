use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn config_root_defaults_to_home_dot_config_gw() {
    // spec: GW-CFG-002, GW-CONFIG-001
    let td = TempDir::new().unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("gw"));
    cmd.env_remove("GW_CONFIG_DIR")
        .env("HOME", td.path())
        .args(["config"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "config_root={}",
            td.path().join(".config").join("gw").to_string_lossy()
        )));
}

