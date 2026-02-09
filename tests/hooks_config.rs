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
fn hooks_shows_global_hooks() {
    let td = TempDir::new().unwrap();
    let repo = td.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();

    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.email", "gw@example.com"]);
    run_git(&repo, &["config", "user.name", "gw"]);
    std::fs::write(repo.join("README.md"), "hi\n").unwrap();
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "init"]);

    let cfg_dir = td.path().join("cfg");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    std::fs::write(
        cfg_dir.join("config.toml"),
        r#"[[hooks]]
command = "echo hook"
"#,
    )
    .unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("gw"));
    cmd.current_dir(&repo)
        .env("GW_CONFIG_DIR", &cfg_dir)
        .args(["hooks"])
        .assert()
        .success()
        .stdout(predicate::str::contains("echo hook"));
}

#[test]
fn config_prints_repo_config_path() {
    let td = TempDir::new().unwrap();
    let repo = td.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();

    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.email", "gw@example.com"]);
    run_git(&repo, &["config", "user.name", "gw"]);
    std::fs::write(repo.join("README.md"), "hi\n").unwrap();
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "init"]);

    let cfg_dir = td.path().join("cfg");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("gw"));
    cmd.current_dir(&repo)
        .env("GW_CONFIG_DIR", &cfg_dir)
        .args(["config"])
        .assert()
        .success()
        .stdout(predicate::str::contains("repos"));
}
