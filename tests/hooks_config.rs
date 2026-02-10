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
    // spec: GW-HOOKS-001, GW-CFG-001
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
    // spec: GW-CONFIG-001, GW-CONFIG-002, GW-CFG-001
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
        .stdout(predicate::str::contains("config_root="))
        .stdout(predicate::str::contains("global_config="))
        .stdout(predicate::str::contains("repo_config="))
        .stdout(predicate::str::contains("repos"));
}

#[test]
fn hooks_shows_repo_hooks_when_configured() {
    // spec: GW-HOOKS-002
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
    let worktrees_dir = td.path().join("worktrees");

    // Create a repo config file via `gw new`, then add a repo hook to it.
    let status = StdCommand::new(assert_cmd::cargo::cargo_bin!("gw"))
        .current_dir(&repo)
        .env("GW_CONFIG_DIR", &cfg_dir)
        .args([
            "new",
            "feat-hooks",
            "--worktrees-dir",
            worktrees_dir.to_str().unwrap(),
            "--no-hooks",
        ])
        .status()
        .expect("failed to run gw new");
    assert!(status.success());

    let out = StdCommand::new(assert_cmd::cargo::cargo_bin!("gw"))
        .current_dir(&repo)
        .env("GW_CONFIG_DIR", &cfg_dir)
        .args(["config"])
        .output()
        .expect("failed to run gw config");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let repo_cfg_line = stdout
        .lines()
        .find(|l| l.starts_with("repo_config="))
        .expect("missing repo_config= line");
    let repo_cfg_path = repo_cfg_line.trim_start_matches("repo_config=").trim();

    let mut cfg_txt = std::fs::read_to_string(repo_cfg_path).unwrap();
    // The repo config is typically emitted with `hooks = []`. TOML forbids mixing `hooks = []`
    // with `[[hooks]]`, so remove the empty array before appending a hook table.
    cfg_txt = cfg_txt.replace("hooks = []", "");
    cfg_txt.push_str(
        r#"
[[hooks]]
command = "echo repo hook"
"#,
    );
    std::fs::write(repo_cfg_path, cfg_txt).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("gw"));
    cmd.current_dir(&repo)
        .env("GW_CONFIG_DIR", &cfg_dir)
        .args(["hooks"])
        .assert()
        .success()
        .stdout(predicate::str::contains("repo: echo repo hook"));
}
