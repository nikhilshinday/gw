# gw

It's 2026, the takeoff is loud, and "running one agent" is the new "opening one browser tab".
If you're shipping with multiple AI agents, you're basically babysitting a small team that never sleeps.

`gw` helps you do that safely: it makes spinning up isolated git worktrees (one per agent/task), jumping between them, and cleaning them up fast enough that you actually use worktrees instead of promising yourself you will.

## Value Prop (Why this exists)

If you context-switch a lot (or delegate work to multiple agents), you need **cheap isolation** and **fast navigation**:

- **One worktree per agent/task**: avoid dependency/build-output bleed, branch collisions, and "wait, which folder is this?"
- **Consistent per-repo worktree locations** with config persisted under `~/.config/gw`
- **Interactive cross-repo picker** (`gw go`) so you can jump to the right sandbox in seconds
- **Hooks on creation** to bootstrap a worktree (install deps, generate files, run checks) consistently
- **Easy cleanup** (`gw rm`) so worktrees don't turn into a graveyard

Under the hood, it's a thin wrapper around `git worktree` with ergonomics designed for high-frequency use.

## Install

### Homebrew (tap)

```bash
brew tap nikhilshinday/tools
brew install gw
```

Note: this currently builds from source (so you will need Rust; Homebrew will install it as a build dependency).

### Cargo (recommended, available everywhere)

```bash
cargo install --git https://github.com/nikhilshinday/gw --locked
```

That puts `gw` in `~/.cargo/bin`.

If you're hacking on the repo locally:

```bash
cargo install --path . --locked
```

## Shell Integration (zsh)

Add this to `~/.zshrc`:

```bash
eval "$(gw init zsh)"
```

Restart your shell (or `source ~/.zshrc`) after adding it.

This wrapper is what lets `gw` / `gw go` / `gw ls` **change your current shell directory**.
Without it, `gw` will just print the selected worktree path (since a subprocess can't `cd` your parent shell).

## Usage

### Create worktree

```bash
gw new my-branch
```

On first use in a repo, it prompts for where to keep worktrees for that repo and stores config under `~/.config/gw`.

Non-interactive override:

```bash
gw new my-branch --worktrees-dir ~/worktrees
```

Worktrees are created under `<worktrees-dir>/<repo-name>/<branch>`.

### List worktrees

```bash
gw list
```

### Go (interactive)

```bash
gw   # or: gw go
```

- Repo picker then worktree picker
- Vim-ish navigation: `j/k`, `gg/G`, `/` to filter, `enter` select, `esc` back, `q` quit
- Quick select hotkeys: `a/s/d/f/...` (single-key then overflow into two-letter combinations)
- In worktree list:
  - `n` create a new worktree (prompts for branch name, then selects it)
  - `Ctrl+D` delete selected worktree (with confirmation; does not delete branch)

### Hooks

Global hooks live in `~/.config/gw/config.toml`:

```toml
[[hooks]]
command = "echo hello from hook"
```

Repo hooks can be added to the repo config (path shown by `gw config`). Hooks run in the new worktree directory after creation.

### Config

```bash
gw config
```

Prints config root + config paths for the current repo.
