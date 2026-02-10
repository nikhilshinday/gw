# New Worktree From Remote Branch / PR URL + In-TUI Help

**Date:** 2026-02-09

## Goal

Improve discoverability and creation flows:

- In the interactive picker, `?` opens a Help Center describing the current screen and available keys.
- `n` should work on both screens:
  - Repo screen: create a new worktree for the highlighted repo.
  - Worktree screen: create a new worktree for the active repo (existing behavior), but with upgraded resolution.
- `gw new` should accept a single “specifier” input that can be either:
  - a branch name (local/remote/new), or
  - a GitHub PR URL (URLs only).

## Single-Field Resolver Rules

Given a `spec` string:

1. If `spec` is a GitHub PR URL (`https://github.com/OWNER/REPO/pull/<N>...`), treat it as a PR.
2. Otherwise treat it as a branch name.

### Remote Selection

- If there is exactly 1 git remote, use it.
- If there are multiple remotes, prompt the user to choose.
- If non-interactive (no TTY) and multiple remotes exist, fail with a clear error.

### Branch Name Handling

- If the branch exists locally: create worktree from the local branch. Do not fetch or compare with remote (even if remote exists).
- If the branch does not exist locally:
  - If the remote has the branch: fetch it, create a local tracking branch, then create the worktree.
  - If the remote does not have it (or no remote configured): create a new branch (from `--base` or `HEAD`) and create the worktree.

### PR URL Handling

- Extract PR number from URL.
- Fetch `refs/pull/<N>/head` from the selected remote into a local branch `pr/<N>`, then create the worktree on `pr/<N>`.
- If the URL does not match the repo (best-effort based on remote URL parsing), fail with a clear message.

## Output / Notifications

In all cases, `gw` prints “what it’s doing” to stderr (remote selection, fetch steps, branch creation/tracking, worktree path).

The picker updates its status line and uses dialoguer prompts while the TUI is suspended.

