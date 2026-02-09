# gw

A small CLI to manage git worktrees with per-repo configuration.

## Install

Recommended (available everywhere):

```bash
cargo install --path /path/to/tools
```

That puts `gw` in `~/.cargo/bin`.

## Shell Integration (zsh)

Add this to `~/.zshrc`:

```bash
eval "$(gw init zsh)"
```

This lets `gw go` change your current shell directory.

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
gw go
```

- Repo picker then worktree picker
- Vim-ish navigation: `j/k`, `gg/G`, `/` to filter, `enter` select, `esc` back, `q` quit
- Quick select hotkeys: `a/s/d/f/...` (single-key then overflow into two-letter combinations)
- In worktree list: press `n` to create a new worktree

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
