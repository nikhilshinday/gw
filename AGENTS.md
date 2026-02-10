# AGENTS.md

This file tracks work performed by an automated coding agent in this repo, so humans can quickly see what changed, why, and how it was validated.

## Work Summary (2026-02-09)

### Skills: captured durable steering workflow

- Added `STEERING.md` to persist human steering/preferences for this repo.
- Added `skills/steering-by-user/SKILL.md` describing a lightweight workflow to turn in-context user steering into explicit, durable rules and apply them consistently.

### Skills: keep steering skill repo-local; log soft preferences too

- Removed the out-of-repo install of `steering-by-user` under `~/.codex/skills`; the skill now lives only in this codebase under `skills/steering-by-user/`.
- Broadened the skill trigger criteria so even suggestions/soft preferences get logged and consolidated into `STEERING.md`.

### Picker-First UX: `gw` opens TUI; `ls` alias; new/delete in-picker

- Made `gw` with no args launch the interactive repo/worktree picker (same behavior as `gw go`).
- Added `gw ls` as an alias for the picker (requested “`ls` can do the same thing as go”).
- Extended the worktree picker screen:
  - `Enter` selects the highlighted worktree (prints path for shell integration to `cd`).
  - `n` prompts for a new branch/worktree name, creates it, and immediately selects it (fast “new+go”).
  - `Ctrl+D` deletes the selected worktree with an in-picker confirmation (`y/n`); branch is preserved.
- Prevented hangs when no TTY is available by failing fast with a clear error.

### README: clarified zshrc eval requirement for `cd`

- Clarified that `eval "$(gw init zsh)"` should live in `~/.zshrc` and is required for `gw` to `cd` the current shell; otherwise it prints the selected path.

Verification:
- `cargo test`

### `gw rm`: delete current worktree + dirty-worktree prompting

- Made `gw rm` accept a positional `PATH` (so `gw rm .` works from inside a worktree).
- If `git worktree remove` fails because the worktree is dirty, `gw` prompts to retry with `--force`; if declined, it offers to `cd` into that worktree instead.
- Updated the zsh shell integration so `gw rm ...` can output a path for the wrapper to `cd`.
- Added regression tests in `tests/remove.rs`.

Verification:
- `cargo test`

### `gw new`: branch/remote/PR URL resolver + TUI Help Center

- Upgraded `gw new` to accept a single specifier: either a branch name or a GitHub PR URL (URLs only).
- If the branch is missing locally but exists on the remote, `gw` fetches it and creates a local tracking branch before creating the worktree.
- Remote selection rule: use the only remote if there is exactly one; otherwise prompt (no special-casing `origin`).
- Extended the picker:
  - `n` works on the repo screen (create/select a worktree for the highlighted repo).
  - `?` opens an in-TUI Help Center describing the current screen and the new-worktree resolver rules.
- Picker resilience: if the repo’s saved `anchor_path` points at a deleted worktree, the picker falls back to listing worktrees via `git_common_dir` and repairs the anchor automatically.
- Added regression tests in `tests/new.rs`.

Verification:
- `cargo test`

### README: clarified positioning + install options

- Added a cheeky opener framing 2026 as “multi-agent takeoff” and positioned `gw` as the tool for safely babysitting multiple agents via isolated worktrees.
- Added an explicit value-prop section:
  - one worktree per agent/task
  - persisted per-repo worktrees location under `~/.config/gw`
  - interactive cross-repo worktree picker (`gw go`)
  - hooks on worktree creation
  - interactive cleanup (`gw rm`)
- Added an installation section with two paths:
  - Homebrew tap (`brew tap nikhilshinday/tools && brew install gw`)
  - Cargo (`cargo install --git ... --locked`, plus local `--path .`)

Relevant commit in this repo:
- `d80295b` docs: clarify multi-agent value prop and add Homebrew install

### Homebrew: created and published a tap

Created a Homebrew tap repo and published a `gw` formula.

Artifacts:
- Tap repo: `nikhilshinday/homebrew-tools`
- Tap name users run: `brew tap nikhilshinday/tools`
- Formula path: `Formula/gw.rb`
- Formula source tarball: `https://github.com/nikhilshinday/gw/archive/refs/tags/v0.1.0.tar.gz`

Notes:
- The formula currently builds from source (depends on `rust` as a build dependency).
- Fixed Homebrew audit issues:
  - Use tag tarball URL (not a bare commit archive URL).
  - Fix test invocation to use `bin/"gw"`.
  - Removed duplicate `--locked` (Homebrew’s `std_cargo_args` already includes it).

### Release tag used by Homebrew

Pushed annotated tag in this repo so the formula can reference a stable source archive:
- Tag: `v0.1.0` (points at `932c6d0`)

## Verification

- Homebrew tap + formula sanity:
  - `brew tap nikhilshinday/tools`
  - `brew install --build-from-source nikhilshinday/tools/gw`
  - `gw --help`

## Follow-ups (optional)

- Add prebuilt binaries (GitHub Releases) to make `brew install` faster (use bottles or release assets + checksums).
- Add a LICENSE file if you intend this to be redistributed broadly (Homebrew formulas commonly expect a license to be declared/discoverable).
