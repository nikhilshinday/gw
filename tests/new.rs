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

#[test]
fn new_can_create_worktree_from_remote_branch_when_missing_locally() {
    let td = TempDir::new().unwrap();
    let remote = td.path().join("remote.git");
    let repo = td.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();

    // Remote.
    run_git(td.path(), &["init", "--bare", remote.to_str().unwrap()]);

    // Local.
    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.email", "gw@example.com"]);
    run_git(&repo, &["config", "user.name", "gw"]);
    std::fs::write(repo.join("README.md"), "hi\n").unwrap();
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "init"]);
    run_git(
        &repo,
        &["remote", "add", "upstream", remote.to_str().unwrap()],
    );

    // Create a remote-only branch without creating it locally.
    run_git(
        &repo,
        &["push", "upstream", "HEAD:refs/heads/feat-remote"],
    );

    let worktrees_dir = td.path().join("worktrees");
    let cfg_dir = td.path().join("cfg");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("gw"));
    cmd.current_dir(&repo)
        .env("GW_CONFIG_DIR", &cfg_dir)
        .args([
            "new",
            "feat-remote",
            "--worktrees-dir",
            worktrees_dir.to_str().unwrap(),
        ])
        .assert()
        .success();

    let wt = worktrees_dir.join("repo").join("feat-remote");
    assert!(wt.exists());

    let branch = git_out(&wt, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(branch.trim(), "feat-remote");

    let upstream = git_out(&wt, &["rev-parse", "--abbrev-ref", "@{u}"]);
    assert_eq!(upstream.trim(), "upstream/feat-remote");
}

#[test]
fn new_accepts_github_pr_url() {
    let td = TempDir::new().unwrap();
    let remote = td.path().join("remote.git");
    let repo = td.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();

    // Remote.
    run_git(td.path(), &["init", "--bare", remote.to_str().unwrap()]);

    // Local.
    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.email", "gw@example.com"]);
    run_git(&repo, &["config", "user.name", "gw"]);
    std::fs::write(repo.join("README.md"), "hi\n").unwrap();
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "init"]);
    run_git(
        &repo,
        &["remote", "add", "upstream", remote.to_str().unwrap()],
    );

    // Simulate a PR ref existing on the remote.
    run_git(&repo, &["push", "upstream", "HEAD:refs/pull/7/head"]);

    let worktrees_dir = td.path().join("worktrees");
    let cfg_dir = td.path().join("cfg");

    let pr_url = "https://github.com/example/repo/pull/7";

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("gw"));
    cmd.current_dir(&repo)
        .env("GW_CONFIG_DIR", &cfg_dir)
        .args([
            "new",
            pr_url,
            "--worktrees-dir",
            worktrees_dir.to_str().unwrap(),
        ])
        .assert()
        .success();

    let wt = worktrees_dir.join("repo").join("pr").join("7");
    assert!(wt.exists());

    let branch = git_out(&wt, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(branch.trim(), "pr/7");
}
