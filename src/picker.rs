use anyhow::Context;
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
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
    git_common_dir: PathBuf,
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
    ConfirmDelete,
    Help,
}

struct AppState {
    screen: Screen,
    mode: Mode,

    status: String,
    pending_delete: Option<PathBuf>,

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

    // If there's no TTY at all, the picker would hang forever waiting for input.
    if !io::stdout().is_terminal() && !io::stderr().is_terminal() {
        anyhow::bail!("no TTY available for interactive picker");
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
            git_common_dir: PathBuf::from(cfg.git_common_dir),
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
        status: "j/k move, gg/G top/bottom, / filter, enter select, n new, ? help, q quit"
            .to_string(),
        pending_delete: None,
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

            if state.mode == Mode::Help {
                let help = help_text(state.screen);
                let area = centered_rect(86, 86, size);
                f.render_widget(Clear, area);
                f.render_widget(
                    Paragraph::new(help)
                        .block(Block::default().borders(Borders::ALL).title("Help"))
                        .wrap(Wrap { trim: false }),
                    area,
                );
            }
        })?;

        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if state.mode == Mode::Help {
                match key.code {
                    KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') => {
                        state.mode = Mode::Normal;
                        state.status = match state.screen {
                            Screen::Repo => "j/k move, gg/G top/bottom, / filter, enter select, n new, ? help, q quit"
                                .to_string(),
                            Screen::Worktree => "j/k move, / filter, enter select, n new, ctrl+d delete, esc back, ? help, q quit"
                                .to_string(),
                        };
                    }
                    _ => {}
                }
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
                    if let Some(sel) =
                        handle_worktree_key(terminal, cfg_root, &mut state, key, &vis_wt_idx)?
                    {
                        return Ok(sel);
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

fn suspend_tui<W: Write>(terminal: &mut Terminal<CrosstermBackend<W>>) {
    disable_raw_mode().ok();
    terminal.backend_mut().execute(LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();
}

fn resume_tui<W: Write>(terminal: &mut Terminal<CrosstermBackend<W>>) -> anyhow::Result<()> {
    enable_raw_mode()?;
    terminal.backend_mut().execute(EnterAlternateScreen)?;
    terminal.clear().ok();
    Ok(())
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
        KeyCode::Char('?') => {
            state.mode = Mode::Help;
            state.status = "press ?/esc/q to close help".to_string();
        }
        KeyCode::Char('/') => {
            state.mode = Mode::Filter;
            state.repo_filter.clear();
            state.status = "filter: type, enter to apply".to_string();
        }
        KeyCode::Char('n') => {
            let repo = vis_repos
                .get(state.repo_selected)
                .context("no repo selected")?;

            let anchor = match load_worktrees(cfg_root, repo) {
                Ok((_, anchor)) => anchor,
                Err(e) => {
                    state.status = format!("failed to load worktrees: {e:#}");
                    return Ok(None);
                }
            };

            suspend_tui(terminal);
            let res: anyhow::Result<Option<PathBuf>> = (|| {
                use dialoguer::{Input, theme::ColorfulTheme};

                let theme = ColorfulTheme::default();
                let spec: String = Input::with_theme(&theme)
                    .with_prompt("Branch name or GitHub PR URL")
                    .interact_text()?;
                let spec = spec.trim().to_string();
                if spec.is_empty() {
                    return Ok(None);
                }

                let wt_path = crate::create_worktree_from_spec(
                    &anchor,
                    cfg_root,
                    &spec,
                    None,
                    None,
                    None,
                    false,
                    true,
                )?;
                Ok(Some(wt_path))
            })();

            resume_tui(terminal)?;

            match res? {
                Some(wt_path) => {
                    persist_repo_anchor(cfg_root, &repo.hash, &wt_path);
                    return Ok(Some(Some(PickerSelection {
                        repo_anchor: anchor,
                        worktree_path: wt_path,
                    })));
                }
                None => state.status = "new cancelled".to_string(),
            }
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
            state.screen = Screen::Worktree;
            state.mode = Mode::Normal;
            state.wt_filter.clear();
            state.wt_selected = 0;
            state.hotkey_buf.clear();
            state.pending_g = false;
            state.status =
                "j/k move, / filter, enter select, n new, ctrl+d delete, esc back, ? help, q quit"
                    .to_string();
            match load_worktrees(cfg_root, repo) {
                Ok((wts, anchor)) => {
                    let mut r = (*repo).clone();
                    r.anchor = anchor;
                    state.active_repo = Some(r);
                    state.wt_entries = wts;
                }
                Err(e) => {
                    state.status = format!("failed to load worktrees: {e:#}");
                    return Ok(None);
                }
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

fn handle_worktree_key<W: Write>(
    terminal: &mut Terminal<CrosstermBackend<W>>,
    cfg_root: &Path,
    state: &mut AppState,
    key: KeyEvent,
    vis_wt_idx: &[usize],
) -> anyhow::Result<Option<Option<PickerSelection>>> {
    let Some(repo) = state.active_repo.clone() else {
        state.screen = Screen::Repo;
        state.mode = Mode::Normal;
        state.status = "no active repo".to_string();
        return Ok(None);
    };

    // Confirmation mode for delete.
    if state.mode == Mode::ConfirmDelete {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let Some(target) = state.pending_delete.take() else {
                    state.mode = Mode::Normal;
                    state.status = "no delete target".to_string();
                    return Ok(None);
                };

                // We already confirmed in the UI; force yes to avoid dialoguer prompts.
                let _ = crate::remove_worktree(&repo.anchor, &target, true, false)?;
                state.mode = Mode::Normal;
                state.status = "worktree removed".to_string();
                match load_worktrees(cfg_root, &repo) {
                    Ok((wts, anchor)) => {
                        state.active_repo = Some(KnownRepo { anchor, ..repo.clone() });
                        state.wt_entries = wts;
                    }
                    Err(e) => {
                        state.status = format!("failed to load worktrees: {e:#}");
                        state.mode = Mode::Normal;
                        return Ok(None);
                    }
                }
                return Ok(None);
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                state.pending_delete = None;
                state.mode = Mode::Normal;
                state.status = "delete cancelled".to_string();
                return Ok(None);
            }
            _ => return Ok(None),
        }
    }

    // Ctrl+D: delete selected worktree (with confirmation).
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char('d') = key.code {
            let i = *vis_wt_idx
                .get(state.wt_selected)
                .context("no worktree selected")?;
            let e = state.wt_entries.get(i).context("no worktree selected")?;
            let target = PathBuf::from(&e.path);
            state.pending_delete = Some(target.clone());
            state.mode = Mode::ConfirmDelete;
            state.status = format!(
                "delete {} ? (y/n)",
                target.to_string_lossy()
            );
            return Ok(None);
        }
    }

    match key.code {
        KeyCode::Char('q') => return Ok(Some(None)),
        KeyCode::Char('?') => {
            state.mode = Mode::Help;
            state.status = "press ?/esc/q to close help".to_string();
        }
        KeyCode::Esc => {
            state.screen = Screen::Repo;
            state.mode = Mode::Normal;
            state.hotkey_buf.clear();
            state.pending_g = false;
            state.status = "j/k move, gg/G top/bottom, / filter, enter select, n new, ? help, q quit"
                .to_string();
        }
        KeyCode::Char('n') => {
            suspend_tui(terminal);

            // Prompt for a new worktree/branch name and create it, then immediately select it.
            let res: anyhow::Result<Option<PathBuf>> = (|| {
                use dialoguer::{Input, theme::ColorfulTheme};

                let theme = ColorfulTheme::default();
                let spec: String = Input::with_theme(&theme)
                    .with_prompt("Branch name or GitHub PR URL")
                    .interact_text()?;
                let spec = spec.trim().to_string();
                if spec.is_empty() {
                    return Ok(None);
                }

                let wt_path = crate::create_worktree_from_spec(
                    &repo.anchor,
                    cfg_root,
                    &spec,
                    None,
                    None,
                    None,
                    false,
                    true,
                )?;
                Ok(Some(wt_path))
            })();

            resume_tui(terminal)?;

            match res? {
                Some(wt_path) => {
                    return Ok(Some(Some(PickerSelection {
                        repo_anchor: repo.anchor,
                        worktree_path: wt_path,
                    })));
                }
                None => {
                    state.status = "new cancelled".to_string();
                }
            }
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
            let wt_path = PathBuf::from(&e.path);
            persist_repo_anchor(cfg_root, &repo.hash, &wt_path);
            return Ok(Some(Some(PickerSelection {
                repo_anchor: repo.anchor,
                worktree_path: wt_path,
            })));
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

fn load_worktrees(
    cfg_root: &Path,
    repo: &KnownRepo,
) -> anyhow::Result<(Vec<WorktreeEntry>, PathBuf)> {
    // First try the configured anchor (fast path).
    if repo.anchor.exists() {
        let out = std::process::Command::new("git")
            .current_dir(&repo.anchor)
            .args(["worktree", "list", "--porcelain"])
            .output();

        if let Ok(out) = out
            && out.status.success()
        {
            let txt = String::from_utf8(out.stdout)?;
            return Ok((parse_worktree_porcelain(&txt), repo.anchor.clone()));
        }
    }

    // Fallback: list worktrees using the repo's common git dir. This works even if the
    // stored anchor points at a deleted worktree.
    let out = std::process::Command::new("git")
        .arg("--git-dir")
        .arg(&repo.git_common_dir)
        .args(["worktree", "list", "--porcelain"])
        .output()?;
    if !out.status.success() {
        anyhow::bail!(
            "git worktree list failed (git_common_dir={}): {}",
            repo.git_common_dir.to_string_lossy(),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let txt = String::from_utf8(out.stdout)?;
    let entries = parse_worktree_porcelain(&txt);

    // Repair the stored anchor to something valid so future opens work without fallback.
    let anchor = if let Some(first) = entries.first() {
        PathBuf::from(&first.path)
    } else {
        // Shouldn't happen, but avoid returning a bogus path.
        repo.anchor.clone()
    };

    if let Some(first) = entries.first() {
        persist_repo_anchor(cfg_root, &repo.hash, Path::new(&first.path));
    }

    Ok((entries, anchor))
}

fn clear_hotkey_buf(state: &mut AppState) {
    state.hotkey_buf.clear();
}

fn reset_chords(state: &mut AppState) {
    state.hotkey_buf.clear();
    state.pending_g = false;
}

fn persist_repo_anchor(cfg_root: &Path, repo_hash: &str, anchor: &Path) {
    let cfg_path = cfg_root
        .join("repos")
        .join(repo_hash)
        .join("config.toml");
    if let Ok(s) = std::fs::read_to_string(&cfg_path)
        && let Ok(mut cfg) = toml::from_str::<RepoConfig>(&s)
    {
        cfg.anchor_path = anchor.to_string_lossy().to_string();
        if let Ok(s2) = toml::to_string_pretty(&cfg) {
            let _ = std::fs::write(cfg_path, s2);
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let w = r.width.saturating_mul(percent_x) / 100;
    let h = r.height.saturating_mul(percent_y) / 100;
    let x = r.x + (r.width.saturating_sub(w)) / 2;
    let y = r.y + (r.height.saturating_sub(h)) / 2;
    Rect { x, y, width: w, height: h }
}

fn help_text(screen: Screen) -> String {
    let common = r#"New worktree input rules (single text field):
- GitHub PR URL only (must be a URL): https://github.com/OWNER/REPO/pull/<N>
- Otherwise, treat input as a branch name.
- If branch exists locally: use it as-is (no fetch / no remote comparison).
- If branch missing locally and exists on remote: fetch it, create a local tracking branch, then create the worktree.
- If branch missing locally and not on remote: create a new branch, then create the worktree.
- Remote selection: if exactly 1 remote, use it; otherwise you will be prompted to choose a remote.
"#;

    match screen {
        Screen::Repo => format!(
            r#"Repo Picker

Keys:
- j/k: move
- gg/G: top/bottom
- /: filter
- enter: open repo's worktrees
- n: create a new worktree for the highlighted repo (then select it)
- ?: help
- q/esc: quit

{common}

Tip: select a repo, then use enter to see its worktrees.
"#
        ),
        Screen::Worktree => format!(
            r#"Worktree Picker

Keys:
- j/k: move
- gg/G: top/bottom
- /: filter
- enter: select highlighted worktree
- n: create a new worktree for this repo (then select it)
- ctrl+d: delete highlighted worktree (confirmation; branch preserved)
- esc: back to repos
- ?: help
- q: quit

{common}
"#
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn load_worktrees_recovers_from_deleted_anchor_via_git_common_dir() {
        let td = TempDir::new().unwrap();
        let repo = td.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();

        run_git(&repo, &["init"]);
        run_git(&repo, &["config", "user.email", "gw@example.com"]);
        run_git(&repo, &["config", "user.name", "gw"]);
        std::fs::write(repo.join("README.md"), "hi\n").unwrap();
        run_git(&repo, &["add", "."]);
        run_git(&repo, &["commit", "-m", "init"]);

        let wt = td.path().join("wt");
        run_git(
            &repo,
            &["worktree", "add", "-b", "feat", wt.to_str().unwrap()],
        );

        let cfg_root = td.path().join("cfg");
        let ctx = crate::RepoContext::detect_from_path(&repo).unwrap();
        let cfg = RepoConfig {
            repo_name: ctx.repo_name.clone(),
            git_common_dir: ctx.git_common_dir.to_string_lossy().to_string(),
            anchor_path: td.path().join("does-not-exist").to_string_lossy().to_string(),
            worktrees_dir: None,
            hooks: Vec::new(),
        };
        crate::save_repo_config(&cfg_root, &ctx, &cfg).unwrap();

        let repos = list_known_repos(&cfg_root).unwrap();
        assert_eq!(repos.len(), 1);
        let known = &repos[0];
        assert!(!known.anchor.exists());

        let (entries, anchor) = load_worktrees(&cfg_root, known).unwrap();
        assert!(!entries.is_empty());
        assert!(anchor.exists());

        // Should have repaired anchor_path in config.
        let repaired = crate::load_repo_config(&cfg_root, &ctx).unwrap();
        assert_eq!(
            repaired.anchor_path,
            entries.first().unwrap().path,
            "expected config anchor_path to be repaired to a valid worktree path"
        );
    }
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
