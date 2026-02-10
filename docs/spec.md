# gw Formal Specification

This document is the repo’s **normative specification** for user-facing behavior.

## Conventions

- Requirements are written using RFC-2119 keywords: **MUST**, **SHOULD**, **MAY**.
- Each requirement has a stable ID like `GW-NEW-003`.
- Any requirement marked `[manual]` is not enforced by automated tests.
- All non-manual requirement IDs **MUST** be referenced by at least one test (enforced by `tests/spec_coverage.rs`).

## Terms

- **Repo root / toplevel**: `git rev-parse --show-toplevel`.
- **Main worktree**: the first entry from `git worktree list --porcelain` (typically the toplevel checkout).
- **Config root**: directory holding gw’s configuration for all repos.
- **Repo config**: per-repo config file under the config root keyed by a hash of the repo’s `git_common_dir`.

## Configuration

- [GW-CFG-001] `gw` MUST use `GW_CONFIG_DIR` as the config root when it is set.
- [GW-CFG-002] If `GW_CONFIG_DIR` is not set, `gw` MUST use `~/.config/gw` as the config root.
- [GW-CFG-003] `gw` MUST store per-repo configuration under `<config_root>/repos/<repo_hash>/config.toml`, where `repo_hash` is derived from the repo’s `git_common_dir`.

## Command: `gw` / `gw go` / `gw ls` (Interactive Picker)

- [GW-PICK-001] Running `gw` with no args MUST behave the same as `gw go` (open the interactive picker).
- [GW-PICK-002] `gw ls` MUST be an alias for `gw go`.
- [GW-PICK-003] If no TTY is available for interactive picker UI, `gw`/`gw go`/`gw ls` MUST fail fast with an error (no hang).
- [GW-PICK-004][manual] When a worktree is selected, the picker MUST print the selected worktree path to stdout.
- [GW-PICK-005] If the picker’s saved per-repo `anchor_path` points to a deleted worktree, the picker MUST still be able to show the repo’s worktree list (self-heal instead of failing).
- [GW-PICK-006] Selecting a worktree MUST update/persist the repo’s `anchor_path` to the selected worktree path (to improve “next time” behavior).
- [GW-PICK-104] The picker footer MUST always include a “commands” hint line describing the available keybindings for the current screen/mode.

- [GW-PICK-101][manual] Repo screen keybindings MUST include navigation (`j/k`, `gg/G`), filter (`/`), open repo (`enter`), new (`n`), help (`?`), quit (`q`/`esc`).
- [GW-PICK-102][manual] Worktree screen keybindings MUST include navigation (`j/k`, `gg/G`), filter (`/`), select (`enter`), new (`n`), delete (`Ctrl+D`), help (`?`), back (`esc`), quit (`q`).
- [GW-PICK-103] Pressing `?` MUST display a help overlay describing the current screen and the “new worktree input rules”.

## Command: `gw init zsh`

- [GW-INIT-001] `gw init zsh` MUST print a zsh function wrapper named `gw()` that calls `command gw ...` to avoid recursion.
- [GW-INIT-002] The wrapper MUST make `gw` (no args), `gw go`, and `gw ls` `cd` the current shell to the selected worktree.
- [GW-INIT-003] The wrapper MUST allow `gw rm ...` to `cd` the current shell when `gw rm` prints a non-empty path.
- [GW-INIT-004] The wrapper MUST allow `gw new ...` to `cd` the current shell when `gw new` prints a non-empty path.

## Command: `gw list`

- [GW-LIST-001] `gw list` MUST list worktrees for the current repository.
- [GW-LIST-002] Each output line MUST be `<path><TAB><branch>`, where `<branch>` is `(detached)` if no branch is associated.

## Command: `gw new`

### Specifier Input

- [GW-NEW-001] `gw new [SPEC]` MUST accept a single optional `SPEC`.
- [GW-NEW-002] If `SPEC` is omitted and no TTY is available, `gw new` MUST fail with a clear error.
- [GW-NEW-003] If `SPEC` is a GitHub PR URL, it MUST be treated as a PR; PRs MUST be accepted **only** via URL form.
- [GW-NEW-004] If `SPEC` is not a GitHub PR URL, it MUST be treated as a branch name (no extra syntax required).

### Remote Selection

- [GW-NEW-010] If the repo has exactly one remote, `gw new` MUST use it when it needs a remote.
- [GW-NEW-011][manual] If the repo has multiple remotes, `gw new` MUST prompt the user to choose a remote when it needs a remote.
- [GW-NEW-012] If the repo has multiple remotes and no TTY is available, `gw new` MUST fail with a clear error rather than prompting.

### Branch Resolution Rules

- [GW-NEW-020] If the branch exists locally, `gw new` MUST create a worktree from the local branch without fetching/comparing against remote.
- [GW-NEW-021] If the branch does not exist locally but exists on the chosen remote, `gw new` MUST fetch it, create a local tracking branch, and create the worktree from that branch.
- [GW-NEW-022] If the branch does not exist locally and does not exist on the chosen remote (or no remote exists), `gw new` MUST create a new branch (from `--base` or `HEAD`) and create the worktree.

### PR URL Rules

- [GW-NEW-030] For a PR URL `https://github.com/OWNER/REPO/pull/N`, `gw new` MUST fetch `refs/pull/N/head` into a local branch `pr/N` and create the worktree from `pr/N`.
- [GW-NEW-031] If the remote URL can be parsed as a GitHub URL, `gw new` MUST reject PR URLs that do not match the selected remote’s `OWNER/REPO`.

### Worktree Location and Config

- [GW-NEW-040] If `--worktrees-dir` is provided, `gw new` MUST persist it (nested by repo name) for future worktree creation in that repo.
- [GW-NEW-041][manual] If no worktrees dir is configured, `gw new` MUST prompt for one (TTY only) and persist it.
- [GW-NEW-042] By default, `gw new` MUST create worktrees under `<worktrees_dir>/<repo_name>/<sanitized_branch_path>`.
- [GW-NEW-043] `--path` MUST override the default worktree path.

### Hooks

- [GW-NEW-050] `gw new` MUST run configured hooks in the new worktree directory unless `--no-hooks` is provided.

### User Feedback

- [GW-NEW-060] `gw new` MUST print what it is doing (e.g. remote selection, fetch steps, branch/tracking actions) to stderr.
- [GW-NEW-070] On success, `gw new` MUST print the created worktree path to stdout (for shell integration to `cd`).

## Command: `gw rm`

- [GW-RM-001] `gw rm` MUST accept a positional `PATH` argument.
- [GW-RM-002] `gw rm --path PATH` MUST also be accepted (equivalent to positional).
- [GW-RM-003] `gw rm` MUST refuse to remove the main worktree.
- [GW-RM-004] Without `--yes`, `gw rm` MUST prompt for confirmation (TTY only); without a TTY it MUST fail with a clear error.
- [GW-RM-005][manual] If `git worktree remove` fails due to modified/untracked files and `--force` is not set, `gw rm` MUST (TTY only) prompt the user to retry with force.
- [GW-RM-006][manual] If the user declines force on a dirty worktree, `gw rm` MUST prompt whether to go to the worktree directory; if accepted, it MUST print the worktree path to stdout (for shell integration to `cd`).
- [GW-RM-007] If `gw rm` is invoked from within the worktree being removed and removal succeeds, it MUST print a safe directory (the main worktree path) to stdout so shell integration can `cd` away from the deleted directory.
- [GW-RM-008] If `git worktree remove` fails due to modified/untracked files and no prompting is possible, `gw rm` MUST fail and include git’s error output.

## Command: `gw config`

- [GW-CONFIG-001] `gw config` MUST print the effective `config_root` and the `global_config` path.
- [GW-CONFIG-002] When run inside a git repo, `gw config` MUST print the `repo_config` path.

## Command: `gw hooks`

- [GW-HOOKS-001] `gw hooks` MUST print configured global hooks as `global: <command>`.
- [GW-HOOKS-002] When run inside a git repo with repo hooks, `gw hooks` MUST print them as `repo: <command>`.

## Command: `gw version`

- [GW-VERSION-001] `gw version` MUST print the current package version to stdout.
