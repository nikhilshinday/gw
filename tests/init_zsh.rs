use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn init_zsh_prints_wrapper_function() {
    // spec: GW-INIT-001, GW-INIT-002, GW-INIT-003
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("gw"));
    cmd.args(["init", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("gw()"))
        .stdout(predicate::str::contains("command gw"))
        .stdout(predicate::str::contains("gw go"))
        .stdout(predicate::str::contains("gw ls"))
        .stdout(predicate::str::contains("gw rm"));
}
