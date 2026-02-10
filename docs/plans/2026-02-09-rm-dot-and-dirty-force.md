# `gw rm .` + Dirty-Worktree Prompt Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make `gw rm .` (and `gw rm <path>`) delete the targeted worktree, and when removal fails due to modified/untracked files, prompt to retry with `--force` or optionally jump to that worktree directory.

**Architecture:** Extend the `rm` subcommand to accept an optional positional `PATH` (keeping `--path` for compatibility). Implement removal via a helper that captures `git worktree remove` stderr, detects the “dirty worktree” failure, and conditionally prompts. For shell UX, allow `rm` to output a path only when we want the shell wrapper to `cd`.

**Tech Stack:** Rust, clap, dialoguer, assert_cmd integration tests.

---

### Task 1: Add failing integration tests for `rm` positional args

**Files:**
- Modify: `tests/remove.rs`

**Step 1: Write the failing tests**

- Add a test verifying `gw rm <path> --yes` removes a non-main worktree (positional path).
- Add a test verifying `gw rm . --yes` works when invoked from inside the target worktree (child process `current_dir` is the worktree).

**Step 2: Run tests to verify they fail**

Run: `cargo test -q`
Expected: FAIL because clap rejects positional arg / `.`.

---

### Task 2: Add failing integration test for dirty worktree behavior (non-TTY)

**Files:**
- Modify: `tests/remove.rs`

**Step 1: Write the failing test**

- Create a worktree, modify a tracked file inside it, then run `gw rm <path> --yes` without `--force`.
- Assert command fails and stderr contains the upstream git error substring: `contains modified or untracked files`.
- Assert worktree path still exists.

**Step 2: Run tests to verify they fail (or validate current behavior)**

Run: `cargo test -q`
Expected: Depending on current behavior, this may already pass; if so, keep as regression coverage.

---

### Task 3: Implement `rm` positional PATH and improve removal logic

**Files:**
- Modify: `src/main.rs`
- Modify: `src/picker.rs` (callsite signature change)

**Step 1: Update clap command**

- Update `Command::Rm` to accept an optional positional `PATH` in addition to `--path`, with conflicts enforced.

**Step 2: Update `remove_worktree` signature**

- Change to return `anyhow::Result<Option<PathBuf>>` where `Some(path)` means “print this for shell wrapper to `cd`”.

**Step 3: Implement robust removal**

- Run `git worktree remove` with output capture.
- If failure stderr contains `contains modified or untracked files` and `--force` not set:
  - If TTY available: prompt to retry with force.
  - If user declines force: prompt to jump to the worktree directory; if yes, return `Ok(Some(target))`.
  - If no TTY: return an error including the git stderr (no prompting).
- If the command is invoked from within the target worktree (current dir inside target) and removal succeeds: return `Ok(Some(main_worktree_path))` so the shell wrapper can land somewhere valid (avoid leaving the shell in a deleted directory).

**Step 4: Run tests**

Run: `cargo test -q`
Expected: PASS.

---

### Task 4: Update zsh shell integration for `rm` to support `cd` on demand

**Files:**
- Modify: `src/main.rs` (init zsh snippet)

**Step 1: Make `rm` capture stdout**

- Change the `rm` branch in the zsh function to capture stdout (like `go`/`ls`) and `cd` if a non-empty path is returned.

**Step 2: Run tests**

Run: `cargo test -q`
Expected: PASS.

---

### Task 5: Update repo steering: bump semver on pushes

**Files:**
- Modify: `STEERING.md`

**Step 1: Add durable instruction**

- Add a “Versioning” convention: when pushing changes intended to be shared/released, bump `Cargo.toml` version (patch by default), and align tags/releases accordingly.

**Step 2: Verify**

Run: `rg -n \"Version\" STEERING.md`
Expected: Steering contains the new rule.

