use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn version_prints_package_version() {
    // spec: GW-VERSION-001
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("gw"));
    cmd.args(["version"])
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}
