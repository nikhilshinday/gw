# AGENTS.md

This file tracks work performed by an automated coding agent in this repo, so humans can quickly see what changed, why, and how it was validated.

## Work Summary (2026-02-09)

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

