use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

mod go_tui;

#[derive(Parser, Debug)]
#[command(name = "gw")]
#[command(about = "Git worktree helper", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Print shell integration snippets
    Init {
        #[arg(value_enum)]
        shell: Shell,
    },
    /// List worktrees for the current repository
    List,
    /// Create a new branch + worktree
    New {
        /// Branch name to create (or check out if it already exists)
        branch: String,
        /// Override the repo worktrees directory and persist it to config
        #[arg(long)]
        worktrees_dir: Option<PathBuf>,
        /// Create the worktree at an explicit path (skips the default <worktrees_dir>/<branch>)
        #[arg(long)]
        path: Option<PathBuf>,
        /// Base ref/commit to create the branch from (default: HEAD)
        #[arg(long)]
        base: Option<String>,
        /// Skip running hooks
        #[arg(long)]
        no_hooks: bool,
    },
    /// Interactive picker to jump between repos/worktrees (prints selected path)
    Go,
    /// Print effective config paths/values for the current repo (if any)
    Config,
    /// Show configured hooks (global + per-repo)
    Hooks,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
enum Shell {
    Zsh,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init { shell: Shell::Zsh } => {
            // A wrapper so `gw go` can `cd` the current shell. `command gw` avoids recursion.
            // Usage: `eval "$(gw init zsh)"`
            println!(
                r#"# gw shell integration (zsh)
gw() {{
  if [[ "$1" == "go" ]]; then
    local dest
    dest="$(command gw go "${{@:2}}")" || return $?
    if [[ -n "$dest" ]]; then
      cd "$dest" || return $?
    fi
  else
    command gw "$@"
  fi
}}"#
            );
        }
        Command::List => {
            let out = std::process::Command::new("git")
                .args(["worktree", "list", "--porcelain"])
                .output()?;
            if !out.status.success() {
                anyhow::bail!(
                    "git worktree list failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }
            let txt = String::from_utf8(out.stdout)?;
            for entry in parse_worktree_porcelain(&txt) {
                let branch = entry.branch.unwrap_or_else(|| "(detached)".to_string());
                println!("{}\t{}", entry.path, branch);
            }
        }
        Command::New {
            branch,
            worktrees_dir,
            path,
            base,
            no_hooks,
        } => {
            let repo = RepoContext::detect_from_cwd()?;
            let cfg_root = config_root()?;
            let global_cfg = load_global_config(&cfg_root)?;

            let mut repo_cfg = load_repo_config(&cfg_root, &repo).unwrap_or_else(|| RepoConfig {
                repo_name: repo.repo_name.clone(),
                git_common_dir: repo.git_common_dir.to_string_lossy().to_string(),
                anchor_path: repo.toplevel.to_string_lossy().to_string(),
                worktrees_dir: None,
                hooks: Vec::new(),
            });

            if let Some(wd) = worktrees_dir {
                // If the user picks a shared base (e.g. ~/worktrees), keep per-repo isolation by nesting.
                let repo_base = wd.join(&repo.repo_name);
                std::fs::create_dir_all(&repo_base)?;
                repo_cfg.worktrees_dir = Some(repo_base.to_string_lossy().to_string());
                save_repo_config(&cfg_root, &repo, &repo_cfg)?;
            }

            let wt_base = match repo_cfg.worktrees_dir.clone() {
                Some(w) => w,
                None => {
                    let picked = prompt_worktrees_dir(&repo)?;
                    std::fs::create_dir_all(&picked)?;
                    repo_cfg.worktrees_dir = Some(picked.to_string_lossy().to_string());
                    save_repo_config(&cfg_root, &repo, &repo_cfg)?;
                    picked.to_string_lossy().to_string()
                }
            };

            let wt_path = match path {
                Some(p) => p,
                None => {
                    let branch_path = sanitize_branch_for_path(&branch);
                    PathBuf::from(wt_base).join(branch_path)
                }
            };
            if let Some(parent) = wt_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let branch_exists = repo.git_show_ref_head(&branch)?;

            let mut args: Vec<String> = vec!["worktree".into(), "add".into()];
            if !branch_exists {
                args.push("-b".into());
                args.push(branch.clone());
            }
            args.push(wt_path.to_string_lossy().to_string());
            if branch_exists {
                args.push(branch.clone());
            } else if let Some(base) = base.clone() {
                args.push(base);
            }

            repo.run_git_strings(&args)?;

            // Update anchor path to the created worktree so `gw go` can find it later.
            repo_cfg.anchor_path = wt_path.to_string_lossy().to_string();
            save_repo_config(&cfg_root, &repo, &repo_cfg)?;

            if !no_hooks {
                let mut hooks = Vec::new();
                hooks.extend(global_cfg.hooks);
                hooks.extend(repo_cfg.hooks);
                run_hooks(&hooks, &repo, &branch, &wt_path)?;
            }
        }
        Command::Go => {
            let repo = RepoContext::detect_from_cwd().ok();
            let cfg_root = config_root()?;
            let selected = go_tui::run_go(&cfg_root, repo)?;
            if let Some(p) = selected {
                println!("{}", p.to_string_lossy());
            } else {
                // Shell wrapper should treat this as cancel.
                std::process::exit(1);
            }
        }
        Command::Config => {
            let cfg_root = config_root()?;
            println!("config_root={}", cfg_root.to_string_lossy());
            println!(
                "global_config={}",
                cfg_root.join("config.toml").to_string_lossy()
            );
            if let Ok(repo) = RepoContext::detect_from_cwd() {
                println!(
                    "repo_config={}",
                    repo_config_path(&cfg_root, &repo).to_string_lossy()
                );
                if let Some(cfg) = load_repo_config(&cfg_root, &repo)
                    && let Some(wd) = cfg.worktrees_dir
                {
                    println!("worktrees_dir={wd}");
                }
            }
        }
        Command::Hooks => {
            let cfg_root = config_root()?;
            let global = load_global_config(&cfg_root)?;
            for h in global.hooks {
                println!("global: {}", h.command);
            }
            if let Ok(repo) = RepoContext::detect_from_cwd()
                && let Some(cfg) = load_repo_config(&cfg_root, &repo)
            {
                for h in cfg.hooks {
                    println!("repo: {}", h.command);
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct WorktreeEntry {
    pub(crate) path: String,
    pub(crate) branch: Option<String>,
}

pub(crate) fn parse_worktree_porcelain(s: &str) -> Vec<WorktreeEntry> {
    let mut entries = Vec::new();
    let mut cur_path: Option<String> = None;
    let mut cur_branch: Option<String> = None;

    for line in s.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            if let Some(path) = cur_path.take() {
                entries.push(WorktreeEntry {
                    path,
                    branch: cur_branch.take(),
                });
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("worktree ") {
            cur_path = Some(rest.to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("branch ") {
            let b = rest.strip_prefix("refs/heads/").unwrap_or(rest).to_string();
            cur_branch = Some(b);
            continue;
        }
    }

    if let Some(path) = cur_path.take() {
        entries.push(WorktreeEntry {
            path,
            branch: cur_branch.take(),
        });
    }

    entries
}

#[derive(Debug, Clone)]
pub(crate) struct RepoContext {
    pub(crate) toplevel: PathBuf,
    pub(crate) git_common_dir: PathBuf,
    pub(crate) repo_name: String,
    pub(crate) repo_hash: String,
}

impl RepoContext {
    pub(crate) fn detect_from_cwd() -> anyhow::Result<Self> {
        Self::detect_from_path(&std::env::current_dir()?)
    }

    pub(crate) fn detect_from_path(path: &Path) -> anyhow::Result<Self> {
        let toplevel = git_stdout(path, &["rev-parse", "--show-toplevel"])?;
        let toplevel = PathBuf::from(toplevel.trim());

        let common = git_stdout(&toplevel, &["rev-parse", "--git-common-dir"])?;
        let common = common.trim();
        let git_common_dir = if Path::new(common).is_absolute() {
            PathBuf::from(common)
        } else {
            toplevel.join(common)
        };
        let git_common_dir = std::fs::canonicalize(&git_common_dir).unwrap_or(git_common_dir);

        let repo_name = toplevel
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "repo".to_string());

        let repo_hash = blake3::hash(git_common_dir.to_string_lossy().as_bytes())
            .to_hex()
            .to_string();

        Ok(Self {
            toplevel,
            git_common_dir,
            repo_name,
            repo_hash,
        })
    }

    pub(crate) fn run_git_strings(&self, args: &[String]) -> anyhow::Result<String> {
        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        git_stdout(&self.toplevel, &args_ref)
    }

    pub(crate) fn git_show_ref_head(&self, branch: &str) -> anyhow::Result<bool> {
        let status = std::process::Command::new("git")
            .current_dir(&self.toplevel)
            .args([
                "show-ref",
                "--verify",
                "--quiet",
                &format!("refs/heads/{branch}"),
            ])
            .status()?;
        Ok(status.success())
    }
}

fn git_stdout(cwd: &Path, args: &[&str]) -> anyhow::Result<String> {
    let out = std::process::Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()?;
    if !out.status.success() {
        anyhow::bail!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8(out.stdout)?)
}

pub(crate) fn config_root() -> anyhow::Result<PathBuf> {
    if let Ok(p) = std::env::var("GW_CONFIG_DIR") {
        return Ok(PathBuf::from(p));
    }
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("could not determine home dir"))?;
    Ok(home.join(".config").join("gw"))
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct Hook {
    pub(crate) command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct GlobalConfig {
    #[serde(default)]
    pub(crate) hooks: Vec<Hook>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RepoConfig {
    pub(crate) repo_name: String,
    pub(crate) git_common_dir: String,
    pub(crate) anchor_path: String,
    pub(crate) worktrees_dir: Option<String>,
    #[serde(default)]
    pub(crate) hooks: Vec<Hook>,
}

pub(crate) fn load_global_config(cfg_root: &Path) -> anyhow::Result<GlobalConfig> {
    let path = cfg_root.join("config.toml");
    if !path.exists() {
        return Ok(GlobalConfig::default());
    }
    let s = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&s)?)
}

fn repo_config_path(cfg_root: &Path, repo: &RepoContext) -> PathBuf {
    cfg_root
        .join("repos")
        .join(&repo.repo_hash)
        .join("config.toml")
}

pub(crate) fn load_repo_config(cfg_root: &Path, repo: &RepoContext) -> Option<RepoConfig> {
    let path = repo_config_path(cfg_root, repo);
    let s = std::fs::read_to_string(path).ok()?;
    toml::from_str(&s).ok()
}

pub(crate) fn save_repo_config(
    cfg_root: &Path,
    repo: &RepoContext,
    cfg: &RepoConfig,
) -> anyhow::Result<()> {
    let path = repo_config_path(cfg_root, repo);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let s = toml::to_string_pretty(cfg)?;
    std::fs::write(path, s)?;
    Ok(())
}

pub(crate) fn sanitize_branch_for_path(branch: &str) -> PathBuf {
    let mut out = PathBuf::new();
    for seg in branch.split('/') {
        if seg.is_empty() {
            continue;
        }
        let cleaned: String = seg
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        out.push(cleaned);
    }
    if out.as_os_str().is_empty() {
        out.push("branch");
    }
    out
}

pub(crate) fn run_hooks(
    hooks: &[Hook],
    repo: &RepoContext,
    branch: &str,
    wt_path: &Path,
) -> anyhow::Result<()> {
    if hooks.is_empty() {
        return Ok(());
    }

    for hook in hooks {
        #[cfg(unix)]
        let mut cmd = {
            let mut c = std::process::Command::new("sh");
            c.args(["-lc", &hook.command]);
            c
        };

        #[cfg(windows)]
        let mut cmd = {
            let mut c = std::process::Command::new("cmd");
            c.args(["/C", &hook.command]);
            c
        };

        let status = cmd
            .current_dir(wt_path)
            .env("GW_WORKTREE_PATH", wt_path.to_string_lossy().to_string())
            .env("GW_BRANCH", branch)
            .env("GW_REPO_ROOT", repo.toplevel.to_string_lossy().to_string())
            .status()?;
        if !status.success() {
            anyhow::bail!("hook failed: {}", hook.command);
        }
    }
    Ok(())
}

pub(crate) fn prompt_worktrees_dir(repo: &RepoContext) -> anyhow::Result<PathBuf> {
    use dialoguer::{Input, Select, theme::ColorfulTheme};

    let theme = ColorfulTheme::default();

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("could not determine home dir"))?;
    let opt1 = home.join("worktrees").join(&repo.repo_name);
    let opt2 = repo
        .toplevel
        .parent()
        .unwrap_or(&repo.toplevel)
        .join(format!("{}-worktrees", repo.repo_name));

    let idx = Select::with_theme(&theme)
        .with_prompt("Where should I put all worktrees for this repo?")
        .items(&[
            format!("{}", opt1.to_string_lossy()),
            format!("{}", opt2.to_string_lossy()),
            "Somewhere else".to_string(),
        ])
        .default(0)
        .interact()?;

    match idx {
        0 => Ok(opt1),
        1 => Ok(opt2),
        _ => {
            let raw: String = Input::with_theme(&theme)
                .with_prompt("Worktrees directory path")
                .interact_text()?;
            let expanded = shellexpand::tilde(&raw).to_string();
            Ok(PathBuf::from(expanded))
        }
    }
}

pub(crate) fn assign_hotkeys(n: usize, pool: &[char]) -> Vec<String> {
    let m = pool.len();
    if m == 0 {
        return vec![];
    }

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        if i < m {
            out.push(pool[i].to_string());
        } else {
            let j = i - m;
            let first = pool[(j / m) % m];
            let second = pool[j % m];
            out.push(format!("{first}{second}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkeys_overflow_to_two_letters_cartesian() {
        let pool: Vec<char> = vec!['a', 's', 'd'];
        let codes = assign_hotkeys(3, &pool);
        assert_eq!(codes, vec!["a", "s", "d"]);

        let codes = assign_hotkeys(4, &pool);
        assert_eq!(codes, vec!["a", "s", "d", "aa"]);

        let codes = assign_hotkeys(7, &pool);
        assert_eq!(codes, vec!["a", "s", "d", "aa", "as", "ad", "sa"]);
    }
}
