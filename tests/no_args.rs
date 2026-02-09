use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn running_without_args_requires_a_tty_for_picker() {
    // In tests, stdout/stderr are pipes (no TTY). The picker should fail fast instead of hanging.
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("gw"));
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("no TTY").or(predicate::str::contains("TTY")));
}

