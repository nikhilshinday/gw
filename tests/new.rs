use assert_cmd::Command;
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

fn git_out(cwd: &Path, args: &[&str]) -> String {
    let out = StdCommand::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("failed to run git");
    assert!(out.status.success(), "git {:?} failed", args);
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn new_creates_worktree_and_branch() {
    let td = TempDir::new().unwrap();
    let repo = td.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();

    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.email", "gw@example.com"]);
    run_git(&repo, &["config", "user.name", "gw"]);

    std::fs::write(repo.join("README.md"), "hi\n").unwrap();
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "init"]);

    let worktrees_dir = td.path().join("worktrees");
    let cfg_dir = td.path().join("cfg");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("gw"));
    cmd.current_dir(&repo)
        .env("GW_CONFIG_DIR", &cfg_dir)
        .args([
            "new",
            "feat1",
            "--worktrees-dir",
            worktrees_dir.to_str().unwrap(),
        ])
        .assert()
        .success();

    let wt = worktrees_dir.join("repo").join("feat1");
    assert!(wt.exists(), "expected worktree dir to exist: {wt:?}");

    // The worktree should be on the requested branch.
    let branch = git_out(&wt, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(branch.trim(), "feat1");

    // Repo config should be written.
    let repos_dir = cfg_dir.join("repos");
    assert!(repos_dir.exists());
}

#[test]
fn new_runs_hooks_when_configured() {
    let td = TempDir::new().unwrap();
    let repo = td.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();

    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.email", "gw@example.com"]);
    run_git(&repo, &["config", "user.name", "gw"]);

    std::fs::write(repo.join("README.md"), "hi\n").unwrap();
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "init"]);

    let worktrees_dir = td.path().join("worktrees");
    let cfg_dir = td.path().join("cfg");

    // Write a global config hook that creates a marker file.
    let global_cfg = cfg_dir.join("config.toml");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    std::fs::write(
        &global_cfg,
        r#"[[hooks]]
command = "echo hook > .gw_hook_ran"
"#,
    )
    .unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("gw"));
    cmd.current_dir(&repo)
        .env("GW_CONFIG_DIR", &cfg_dir)
        .args([
            "new",
            "feat2",
            "--worktrees-dir",
            worktrees_dir.to_str().unwrap(),
        ])
        .assert()
        .success();

    let wt = worktrees_dir.join("repo").join("feat2");
    assert!(wt.join(".gw_hook_ran").exists());
}
