use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use std::process::Command as StdCommand;
use tempfile::TempDir;

fn run_git(cwd: &Path, args: &[&str]) {
    let status = StdCommand::new("git")
        .current_dir(cwd)
        .args(args)
        .status()
        .expect("failed to run git");
    assert!(status.success(), "git {:?} failed", args);
}

#[test]
fn list_shows_worktrees_in_repo() {
    let td = TempDir::new().unwrap();
    let repo = td.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();

    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.email", "gw@example.com"]);
    run_git(&repo, &["config", "user.name", "gw"]);

    std::fs::write(repo.join("README.md"), "hi\n").unwrap();
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "init"]);

    let wt = td.path().join("wt");
    run_git(
        &repo,
        &["worktree", "add", "-b", "feat", wt.to_str().unwrap()],
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("gw"));
    cmd.current_dir(&repo)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains(wt.to_string_lossy().as_ref()))
        .stdout(predicate::str::contains("feat"));
}
