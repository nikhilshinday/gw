use anyhow::Context;
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use std::collections::HashMap;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::{
    RepoConfig, RepoContext, WorktreeEntry, assign_hotkeys, load_repo_config,
    parse_worktree_porcelain, save_repo_config,
};

#[derive(Debug, Clone)]
struct KnownRepo {
    hash: String,
    name: String,
    anchor: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Repo,
    Worktree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Normal,
    Filter,
}

struct AppState {
    screen: Screen,
    mode: Mode,

    status: String,

    repo_filter: String,
    repo_selected: usize,
    repo_list_state: ListState,

    active_repo: Option<KnownRepo>,

    wt_filter: String,
    wt_selected: usize,
    wt_list_state: ListState,
    wt_entries: Vec<WorktreeEntry>,

    hotkey_buf: String,
    last_hotkey_at: Instant,

    pending_g: bool,
    last_g_at: Instant,
}

#[derive(Debug, Clone)]
pub(crate) struct PickerSelection {
    pub(crate) repo_anchor: PathBuf,
    pub(crate) worktree_path: PathBuf,
}

pub(crate) fn pick_worktree(
    cfg_root: &Path,
    current_repo: Option<RepoContext>,
) -> anyhow::Result<Option<PickerSelection>> {
    let mut repos = list_known_repos(cfg_root)?;

    // If invoked inside a repo and it's not known yet, add it (persisting a stub config).
    if let Some(repo) = &current_repo
        && load_repo_config(cfg_root, repo).is_none()
    {
        let stub = RepoConfig {
            repo_name: repo.repo_name.clone(),
            git_common_dir: repo.git_common_dir.to_string_lossy().to_string(),
            anchor_path: repo.toplevel.to_string_lossy().to_string(),
            worktrees_dir: None,
            hooks: Vec::new(),
        };
        save_repo_config(cfg_root, repo, &stub)?;
        repos = list_known_repos(cfg_root)?;
    }

    if repos.is_empty() {
        return Ok(None);
    }

    // In shell command-substitution, stdout is a pipe and the TUI would be invisible.
    // Draw the UI to stderr in that case.
    let use_stderr = !io::stdout().is_terminal() && io::stderr().is_terminal();
    if use_stderr {
        pick_with_terminal(io::stderr(), cfg_root, &repos, current_repo.as_ref())
    } else {
        pick_with_terminal(io::stdout(), cfg_root, &repos, current_repo.as_ref())
    }
}

fn pick_with_terminal<W: Write>(
    mut w: W,
    cfg_root: &Path,
    repos: &[KnownRepo],
    current_repo: Option<&RepoContext>,
) -> anyhow::Result<Option<PickerSelection>> {
    enable_raw_mode()?;
    w.execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(w);
    let mut terminal = Terminal::new(backend)?;

    let res = picker_loop(&mut terminal, cfg_root, repos, current_repo);

    disable_raw_mode().ok();
    terminal.backend_mut().execute(LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    res
}

fn list_known_repos(cfg_root: &Path) -> anyhow::Result<Vec<KnownRepo>> {
    let repos_dir = cfg_root.join("repos");
    if !repos_dir.exists() {
        return Ok(Vec::new());
    }

    let mut repos = Vec::new();
    for ent in std::fs::read_dir(&repos_dir).context("read repos dir")? {
        let ent = ent?;
        if !ent.file_type()?.is_dir() {
            continue;
        }
        let hash = ent.file_name().to_string_lossy().to_string();
        let cfg_path = ent.path().join("config.toml");
        let s = match std::fs::read_to_string(&cfg_path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let cfg: RepoConfig = match toml::from_str(&s) {
            Ok(c) => c,
            Err(_) => continue,
        };
        repos.push(KnownRepo {
            hash,
            name: cfg.repo_name,
            anchor: PathBuf::from(cfg.anchor_path),
        });
    }

    repos.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(repos)
}

fn picker_loop<W: Write>(
    terminal: &mut Terminal<CrosstermBackend<W>>,
    cfg_root: &Path,
    repos: &[KnownRepo],
    current_repo: Option<&RepoContext>,
) -> anyhow::Result<Option<PickerSelection>> {
    let mut state = AppState {
        screen: Screen::Repo,
        mode: Mode::Normal,
        status: "j/k move, gg/G top/bottom, / filter, enter select, q quit".to_string(),
        repo_filter: String::new(),
        repo_selected: 0,
        repo_list_state: ListState::default(),
        active_repo: None,
        wt_filter: String::new(),
        wt_selected: 0,
        wt_list_state: ListState::default(),
        wt_entries: Vec::new(),
        hotkey_buf: String::new(),
        last_hotkey_at: Instant::now(),
        pending_g: false,
        last_g_at: Instant::now(),
    };

    if let Some(cur) = current_repo
        && let Some(idx) = repos.iter().position(|r| r.hash == cur.repo_hash)
    {
        state.repo_selected = idx;
    }

    loop {
        if !state.hotkey_buf.is_empty()
            && state.last_hotkey_at.elapsed() > Duration::from_millis(1500)
        {
            state.hotkey_buf.clear();
        }
        if state.pending_g && state.last_g_at.elapsed() > Duration::from_millis(600) {
            state.pending_g = false;
        }

        let (vis_repos, repo_codes, repo_code_map) = visible_repos(repos, &state.repo_filter);
        state.repo_selected = state.repo_selected.min(vis_repos.len().saturating_sub(1));
        state.repo_list_state.select(Some(state.repo_selected));

        let vis_wt_idx = visible_worktrees_idx(&state.wt_entries, &state.wt_filter);
        state.wt_selected = state.wt_selected.min(vis_wt_idx.len().saturating_sub(1));
        state.wt_list_state.select(Some(state.wt_selected));

        terminal.draw(|f| {
            let size = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(2),
                    Constraint::Length(2),
                ])
                .split(size);

            let title = match state.screen {
                Screen::Repo => "gw: repos",
                Screen::Worktree => "gw: worktrees",
            };

            let filter_txt = match state.screen {
                Screen::Repo => format!("/{}", state.repo_filter),
                Screen::Worktree => format!("/{}", state.wt_filter),
            };

            let header = Paragraph::new(Line::from(vec![
                Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("    "),
                Span::styled(filter_txt, Style::default().fg(Color::DarkGray)),
                Span::raw("    "),
                Span::styled(
                    state.hotkey_buf.clone(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]))
            .block(Block::default().borders(Borders::ALL));
            f.render_widget(header, chunks[0]);

            match state.screen {
                Screen::Repo => {
                    let items: Vec<ListItem> = vis_repos
                        .iter()
                        .enumerate()
                        .map(|(i, r)| {
                            let code = repo_codes.get(i).cloned().unwrap_or_default();
                            let line = Line::from(vec![
                                Span::styled(
                                    format!("[{code}] "),
                                    Style::default().fg(Color::Cyan),
                                ),
                                Span::raw(&r.name),
                                Span::raw("  "),
                                Span::styled(
                                    r.anchor.to_string_lossy(),
                                    Style::default().fg(Color::DarkGray),
                                ),
                            ]);
                            ListItem::new(line)
                        })
                        .collect();
                    let list = List::new(items)
                        .block(Block::default().borders(Borders::ALL).title("Repos"))
                        .highlight_style(
                            Style::default()
                                .bg(Color::Blue)
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        );
                    f.render_stateful_widget(list, chunks[1], &mut state.repo_list_state);
                }
                Screen::Worktree => {
                    let pool = hotkey_pool_worktrees();
                    let codes = assign_hotkeys(vis_wt_idx.len(), &pool);
                    let items: Vec<ListItem> = vis_wt_idx
                        .iter()
                        .enumerate()
                        .map(|(i, idx)| {
                            let code = codes.get(i).cloned().unwrap_or_default();
                            let e = &state.wt_entries[*idx];
                            let branch =
                                e.branch.clone().unwrap_or_else(|| "(detached)".to_string());
                            let line = Line::from(vec![
                                Span::styled(
                                    format!("[{code}] "),
                                    Style::default().fg(Color::Cyan),
                                ),
                                Span::raw(&e.path),
                                Span::raw("  "),
                                Span::styled(branch, Style::default().fg(Color::Green)),
                            ]);
                            ListItem::new(line)
                        })
                        .collect();
                    let list = List::new(items)
                        .block(Block::default().borders(Borders::ALL).title("Worktrees"))
                        .highlight_style(
                            Style::default()
                                .bg(Color::Blue)
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        );
                    f.render_stateful_widget(list, chunks[1], &mut state.wt_list_state);
                }
            }

            let footer =
                Paragraph::new(state.status.clone()).block(Block::default().borders(Borders::ALL));
            f.render_widget(footer, chunks[2]);
        })?;

        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if state.mode == Mode::Filter && handle_filter_mode(&mut state, key) {
                continue;
            }

            match state.screen {
                Screen::Repo => {
                    if let Some(sel) = handle_repo_key(
                        terminal,
                        cfg_root,
                        &mut state,
                        key,
                        &vis_repos,
                        &repo_codes,
                        &repo_code_map,
                    )? {
                        return Ok(sel);
                    }
                }
                Screen::Worktree => {
                    if let Some(sel) = handle_worktree_key(&mut state, key, &vis_wt_idx)?
                        && let Some(repo) = state.active_repo.clone()
                    {
                        return Ok(Some(PickerSelection {
                            repo_anchor: repo.anchor,
                            worktree_path: sel,
                        }));
                    }
                }
            }
        }
    }
}

fn visible_repos<'a>(
    repos: &'a [KnownRepo],
    filter: &str,
) -> (Vec<&'a KnownRepo>, Vec<String>, HashMap<String, usize>) {
    let f = filter.to_lowercase();
    let vis: Vec<&KnownRepo> = repos
        .iter()
        .filter(|r| {
            if f.is_empty() {
                true
            } else {
                format!("{} {}", r.name, r.anchor.to_string_lossy())
                    .to_lowercase()
                    .contains(&f)
            }
        })
        .collect();

    let pool = hotkey_pool_repos();
    let codes = assign_hotkeys(vis.len(), &pool);
    let mut map = HashMap::new();
    for (i, c) in codes.iter().enumerate() {
        map.insert(c.clone(), i);
    }

    (vis, codes, map)
}

fn visible_worktrees_idx(entries: &[WorktreeEntry], filter: &str) -> Vec<usize> {
    let f = filter.to_lowercase();
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            if f.is_empty() {
                true
            } else {
                format!("{} {}", e.path, e.branch.clone().unwrap_or_default())
                    .to_lowercase()
                    .contains(&f)
            }
        })
        .map(|(i, _)| i)
        .collect()
}

fn handle_filter_mode(state: &mut AppState, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Esc => {
            state.mode = Mode::Normal;
            state.status = "cancelled filter".to_string();
            true
        }
        KeyCode::Enter => {
            state.mode = Mode::Normal;
            state.status = "filter applied".to_string();
            true
        }
        KeyCode::Backspace => {
            match state.screen {
                Screen::Repo => {
                    state.repo_filter.pop();
                }
                Screen::Worktree => {
                    state.wt_filter.pop();
                }
            }
            true
        }
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                return false;
            }
            match state.screen {
                Screen::Repo => state.repo_filter.push(c),
                Screen::Worktree => state.wt_filter.push(c),
            }
            true
        }
        _ => false,
    }
}

fn handle_repo_key<W: Write>(
    terminal: &mut Terminal<CrosstermBackend<W>>,
    cfg_root: &Path,
    state: &mut AppState,
    key: KeyEvent,
    vis_repos: &[&KnownRepo],
    repo_codes: &[String],
    repo_code_map: &HashMap<String, usize>,
) -> anyhow::Result<Option<Option<PickerSelection>>> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => return Ok(Some(None)),
        KeyCode::Char('/') => {
            state.mode = Mode::Filter;
            state.repo_filter.clear();
            state.status = "filter: type, enter to apply".to_string();
        }
        KeyCode::Char('j') => {
            state.repo_selected = (state.repo_selected + 1).min(vis_repos.len().saturating_sub(1));
            reset_chords(state);
        }
        KeyCode::Char('k') => {
            state.repo_selected = state.repo_selected.saturating_sub(1);
            reset_chords(state);
        }
        KeyCode::Char('G') => {
            state.repo_selected = vis_repos.len().saturating_sub(1);
            reset_chords(state);
        }
        KeyCode::Char('g') => {
            if state.pending_g {
                state.repo_selected = 0;
                state.pending_g = false;
            } else {
                state.pending_g = true;
                state.last_g_at = Instant::now();
            }
            clear_hotkey_buf(state);
        }
        KeyCode::Enter => {
            let repo = vis_repos
                .get(state.repo_selected)
                .context("no repo selected")?;
            state.active_repo = Some((*repo).clone());
            state.screen = Screen::Worktree;
            state.mode = Mode::Normal;
            state.wt_filter.clear();
            state.wt_selected = 0;
            state.hotkey_buf.clear();
            state.pending_g = false;
            state.status = "j/k move, / filter, enter select, esc back".to_string();
            state.wt_entries = load_worktrees(repo)?;

            if let Ok(ctx) = RepoContext::detect_from_path(&repo.anchor)
                && let Some(mut cfg) = load_repo_config(cfg_root, &ctx)
            {
                cfg.anchor_path = repo.anchor.to_string_lossy().to_string();
                let _ = save_repo_config(cfg_root, &ctx, &cfg);
            }

            terminal.clear().ok();
        }
        KeyCode::Char(c) => {
            if is_repo_hotkey(c) {
                push_hotkey(state, c);

                if let Some(sel) = repo_code_map.get(&state.hotkey_buf).copied() {
                    state.repo_selected = sel;
                    if state.hotkey_buf.len() >= 2 {
                        state.hotkey_buf.clear();
                    }
                } else if !has_prefix(&state.hotkey_buf, repo_codes) || state.hotkey_buf.len() >= 2
                {
                    state.hotkey_buf.clear();
                }
            }
        }
        _ => {}
    }

    Ok(None)
}

fn handle_worktree_key(
    state: &mut AppState,
    key: KeyEvent,
    vis_wt_idx: &[usize],
) -> anyhow::Result<Option<PathBuf>> {
    match key.code {
        KeyCode::Char('q') => return Ok(None),
        KeyCode::Esc => {
            state.screen = Screen::Repo;
            state.mode = Mode::Normal;
            state.hotkey_buf.clear();
            state.pending_g = false;
            state.status = "j/k move, gg/G top/bottom, / filter, enter select, q quit".to_string();
        }
        KeyCode::Char('/') => {
            state.mode = Mode::Filter;
            state.wt_filter.clear();
            state.status = "filter: type, enter to apply".to_string();
        }
        KeyCode::Char('j') => {
            state.wt_selected = (state.wt_selected + 1).min(vis_wt_idx.len().saturating_sub(1));
            reset_chords(state);
        }
        KeyCode::Char('k') => {
            state.wt_selected = state.wt_selected.saturating_sub(1);
            reset_chords(state);
        }
        KeyCode::Char('G') => {
            state.wt_selected = vis_wt_idx.len().saturating_sub(1);
            reset_chords(state);
        }
        KeyCode::Char('g') => {
            if state.pending_g {
                state.wt_selected = 0;
                state.pending_g = false;
            } else {
                state.pending_g = true;
                state.last_g_at = Instant::now();
            }
            clear_hotkey_buf(state);
        }
        KeyCode::Enter => {
            let i = *vis_wt_idx
                .get(state.wt_selected)
                .context("no worktree selected")?;
            let e = state.wt_entries.get(i).context("no worktree selected")?;
            return Ok(Some(PathBuf::from(&e.path)));
        }
        KeyCode::Char(c) => {
            if is_worktree_hotkey(c) {
                let pool = hotkey_pool_worktrees();
                let codes = assign_hotkeys(vis_wt_idx.len(), &pool);
                let mut map = HashMap::new();
                for (i, code) in codes.iter().enumerate() {
                    map.insert(code.clone(), i);
                }

                push_hotkey(state, c);

                if let Some(sel) = map.get(&state.hotkey_buf).copied() {
                    state.wt_selected = sel;
                    if state.hotkey_buf.len() >= 2 {
                        state.hotkey_buf.clear();
                    }
                } else if !has_prefix(&state.hotkey_buf, &codes) || state.hotkey_buf.len() >= 2 {
                    state.hotkey_buf.clear();
                }
            }
        }
        _ => {}
    }

    Ok(None)
}

fn load_worktrees(repo: &KnownRepo) -> anyhow::Result<Vec<WorktreeEntry>> {
    let out = std::process::Command::new("git")
        .current_dir(&repo.anchor)
        .args(["worktree", "list", "--porcelain"])
        .output()?;
    if !out.status.success() {
        anyhow::bail!(
            "git worktree list failed for {}: {}",
            repo.anchor.to_string_lossy(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let txt = String::from_utf8(out.stdout)?;
    Ok(parse_worktree_porcelain(&txt))
}

fn clear_hotkey_buf(state: &mut AppState) {
    state.hotkey_buf.clear();
}

fn reset_chords(state: &mut AppState) {
    state.hotkey_buf.clear();
    state.pending_g = false;
}

fn push_hotkey(state: &mut AppState, c: char) {
    if state.hotkey_buf.len() >= 2 {
        state.hotkey_buf.clear();
    }
    state.hotkey_buf.push(c);
    state.last_hotkey_at = Instant::now();
}

fn has_prefix(buf: &str, codes: &[String]) -> bool {
    codes.iter().any(|c| c.starts_with(buf))
}

fn is_repo_hotkey(c: char) -> bool {
    hotkey_pool_repos().contains(&c)
}

fn is_worktree_hotkey(c: char) -> bool {
    hotkey_pool_worktrees().contains(&c)
}

fn hotkey_pool_repos() -> Vec<char> {
    vec![
        'a', 's', 'd', 'f', 'h', 'l', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p', 'z', 'x', 'c',
        'v', 'b', 'n', 'm',
    ]
}

fn hotkey_pool_worktrees() -> Vec<char> {
    vec![
        'a', 's', 'd', 'f', 'h', 'l', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p', 'z', 'x', 'c',
        'v', 'b', 'm',
    ]
}
