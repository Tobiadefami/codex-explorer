use std::{
    io,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::{
    codex::ParsedMessage,
    db::{Database, SessionDetail, SessionSummary},
    indexer::{self, ReindexReport},
};

const TUI_RESULT_LIMIT: usize = 50;
const REFRESH_TICK: Duration = Duration::from_millis(120);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectScope {
    CurrentDirectory,
    AllProjects,
}

pub struct TuiState {
    query: String,
    summaries: Vec<SessionSummary>,
    selected_index: Option<usize>,
    limit: usize,
    current_dir: Option<PathBuf>,
    scope: ProjectScope,
    scope_note: Option<String>,
    preview_scroll: usize,
}

impl TuiState {
    pub fn load(database: &Database, limit: usize) -> Result<Self> {
        let summaries = database.list_sessions(limit)?;
        let selected_index = first_index(&summaries);

        Ok(Self {
            query: String::new(),
            summaries,
            selected_index,
            limit,
            current_dir: None,
            scope: ProjectScope::AllProjects,
            scope_note: None,
            preview_scroll: 0,
        })
    }

    pub fn load_scoped(database: &Database, current_dir: PathBuf, limit: usize) -> Result<Self> {
        let cwd = current_dir.display().to_string();
        let summaries = database.list_sessions_for_cwd(&cwd, limit)?;
        if summaries.is_empty() {
            let mut state = Self::load(database, limit)?;
            state.current_dir = Some(current_dir);
            state.scope_note = Some("No sessions for current directory".to_string());
            return Ok(state);
        }

        let selected_index = first_index(&summaries);
        Ok(Self {
            query: String::new(),
            summaries,
            selected_index,
            limit,
            current_dir: Some(current_dir),
            scope: ProjectScope::CurrentDirectory,
            scope_note: None,
            preview_scroll: 0,
        })
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn summaries(&self) -> &[SessionSummary] {
        &self.summaries
    }

    pub fn scope(&self) -> &ProjectScope {
        &self.scope
    }

    pub fn scope_label(&self) -> &'static str {
        match self.scope() {
            ProjectScope::CurrentDirectory => "Current directory",
            ProjectScope::AllProjects => "All projects",
        }
    }

    pub fn scope_note(&self) -> Option<&str> {
        self.scope_note.as_deref()
    }

    pub fn preview_scroll(&self) -> usize {
        self.preview_scroll
    }

    pub fn set_query(&mut self, database: &Database, query: String) -> Result<()> {
        self.query = query;
        self.reload(database)
    }

    pub fn reload(&mut self, database: &Database) -> Result<()> {
        self.summaries = self.load_summaries(database)?;
        self.selected_index = first_index(&self.summaries);
        self.preview_scroll = 0;
        Ok(())
    }

    fn load_summaries(&self, database: &Database) -> Result<Vec<SessionSummary>> {
        match self.scope {
            ProjectScope::CurrentDirectory => {
                let Some(current_dir) = &self.current_dir else {
                    return self.load_all_summaries(database);
                };
                let cwd = current_dir.display().to_string();
                if self.query.trim().is_empty() {
                    database.list_sessions_for_cwd(&cwd, self.limit)
                } else {
                    database.search_sessions_for_cwd(&cwd, &self.query, self.limit)
                }
            }
            ProjectScope::AllProjects => self.load_all_summaries(database),
        }
    }

    fn load_all_summaries(&self, database: &Database) -> Result<Vec<SessionSummary>> {
        if self.query.trim().is_empty() {
            database.list_sessions(self.limit)
        } else {
            database.search_sessions(&self.query, self.limit)
        }
    }

    pub fn show_all_projects(&mut self, database: &Database) -> Result<()> {
        self.scope = ProjectScope::AllProjects;
        self.scope_note = None;
        self.reload(database)
    }

    pub fn show_current_directory(&mut self, database: &Database) -> Result<()> {
        if self.current_dir.is_none() {
            return self.show_all_projects(database);
        }

        self.scope = ProjectScope::CurrentDirectory;
        self.scope_note = None;
        self.reload(database)?;
        if self.summaries.is_empty() && self.query.trim().is_empty() {
            self.scope = ProjectScope::AllProjects;
            self.scope_note = Some("No sessions for current directory".to_string());
            self.reload(database)?;
        }
        Ok(())
    }

    pub fn move_down(&mut self) {
        let Some(selected_index) = self.selected_index else {
            return;
        };
        let last_index = self.summaries.len().saturating_sub(1);
        self.selected_index = Some((selected_index + 1).min(last_index));
        self.preview_scroll = 0;
    }

    pub fn move_up(&mut self) {
        let Some(selected_index) = self.selected_index else {
            return;
        };
        self.selected_index = Some(selected_index.saturating_sub(1));
        self.preview_scroll = 0;
    }

    pub fn scroll_preview_down(&mut self) {
        self.preview_scroll = self.preview_scroll.saturating_add(1);
    }

    pub fn scroll_preview_page_down(&mut self) {
        self.preview_scroll = self.preview_scroll.saturating_add(8);
    }

    pub fn scroll_preview_up(&mut self) {
        self.preview_scroll = self.preview_scroll.saturating_sub(1);
    }

    pub fn scroll_preview_page_up(&mut self) {
        self.preview_scroll = self.preview_scroll.saturating_sub(8);
    }

    pub fn scroll_preview_top(&mut self) {
        self.preview_scroll = 0;
    }

    pub fn selected_summary(&self) -> Option<&SessionSummary> {
        self.selected_index
            .and_then(|selected_index| self.summaries.get(selected_index))
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    pub fn selected_session_id(&self) -> Option<&str> {
        self.selected_summary()
            .map(|summary| summary.session_id.as_str())
    }

    pub fn mode_label(&self) -> &'static str {
        if self.query.trim().is_empty() {
            self.scope_label()
        } else {
            "Search"
        }
    }

    pub fn result_label(&self) -> String {
        let count = self.summaries.len();
        let noun = if self.query.trim().is_empty() {
            if count == 1 {
                "session"
            } else {
                "sessions"
            }
        } else if count == 1 {
            "match"
        } else {
            "matches"
        };

        format!("{count} {noun}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshStatus {
    Running {
        spinner_index: usize,
    },
    Complete {
        scanned_files: usize,
        indexed_sessions: usize,
    },
    Failed {
        message: String,
    },
}

impl RefreshStatus {
    pub fn running() -> Self {
        Self::Running { spinner_index: 0 }
    }

    pub fn complete(scanned_files: usize, indexed_sessions: usize) -> Self {
        Self::Complete {
            scanned_files,
            indexed_sessions,
        }
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
        }
    }

    pub fn tick(&mut self) {
        if let Self::Running { spinner_index } = self {
            *spinner_index = (*spinner_index + 1) % REFRESH_SPINNER.len();
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Running { spinner_index } => {
                format!("Refreshing {}", REFRESH_SPINNER[*spinner_index])
            }
            Self::Complete {
                scanned_files,
                indexed_sessions,
            } => format!("Refreshed {indexed_sessions} sessions from {scanned_files} files"),
            Self::Failed { message } => format!("Refresh failed: {message}"),
        }
    }
}

const REFRESH_SPINNER: &[&str] = &["|", "/", "-", "\\"];

type RefreshResult = std::result::Result<ReindexReport, String>;

fn first_index(summaries: &[SessionSummary]) -> Option<usize> {
    if summaries.is_empty() {
        None
    } else {
        Some(0)
    }
}

pub fn short_session_id(session_id: &str) -> String {
    if session_id.len() <= 16 {
        return session_id.to_string();
    }

    let prefix = &session_id[..8];
    let suffix = &session_id[session_id.len() - 4..];
    format!("{prefix}...{suffix}")
}

pub fn compact_path(path: &str) -> String {
    if path == "/" {
        return path.to_string();
    }

    Path::new(path)
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .filter(|file_name| !file_name.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.to_string())
}

pub fn compact_timestamp(timestamp: &str) -> String {
    if timestamp.len() >= 16 && timestamp.as_bytes().get(10) == Some(&b'T') {
        format!("{} {}", &timestamp[..10], &timestamp[11..16])
    } else {
        timestamp.to_string()
    }
}

pub fn empty_results_message(query: &str) -> String {
    if query.trim().is_empty() {
        "No sessions indexed yet".to_string()
    } else {
        format!("No sessions match {:?}", query)
    }
}

pub fn empty_state_message(query: &str, refresh_status: &RefreshStatus) -> String {
    if matches!(refresh_status, RefreshStatus::Running { .. }) {
        if query.trim().is_empty() {
            "Refreshing sessions...".to_string()
        } else {
            format!("Refreshing matches for {:?}...", query)
        }
    } else {
        empty_results_message(query)
    }
}

pub fn meaningful_preview_messages(
    messages: &[ParsedMessage],
    limit: usize,
) -> Vec<&ParsedMessage> {
    messages
        .iter()
        .filter(|message| is_meaningful_message(&message.text))
        .take(limit)
        .collect()
}

fn is_meaningful_message(text: &str) -> bool {
    let text = text.trim_start();
    if text.is_empty() {
        return false;
    }

    const BOOTSTRAP_PREFIXES: &[&str] = &[
        "<environment_context",
        "<permissions instructions>",
        "<apps_instructions>",
        "<skills_instructions>",
        "<plugins_instructions>",
        "<collaboration_mode>",
        "# AGENTS.md instructions",
        "# CLAUDE.md instructions",
        "# GEMINI.md instructions",
    ];

    !BOOTSTRAP_PREFIXES
        .iter()
        .any(|prefix| text.starts_with(prefix))
}

pub fn run(
    database: &Database,
    db_path: PathBuf,
    sessions_dir: PathBuf,
    current_dir: PathBuf,
) -> Result<Option<String>> {
    let _terminal_guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut state = TuiState::load_scoped(database, current_dir, TUI_RESULT_LIMIT)?;
    let refresh_results = start_refresh(db_path, sessions_dir);
    let mut refresh_status = RefreshStatus::running();

    loop {
        apply_refresh_results(database, &mut state, &mut refresh_status, &refresh_results)?;
        let preview = selected_preview(database, &state)?;
        terminal.draw(|frame| render(frame, &state, &refresh_status, preview.as_ref()))?;

        if event::poll(REFRESH_TICK)? {
            let Event::Key(key_event) = event::read()? else {
                continue;
            };
            match handle_key(database, &mut state, key_event)? {
                TuiAction::Continue => {}
                TuiAction::Quit => return Ok(None),
                TuiAction::Resume(session_id) => return Ok(Some(session_id)),
            }
        } else {
            refresh_status.tick();
        }
    }
}

fn start_refresh(db_path: PathBuf, sessions_dir: PathBuf) -> Receiver<RefreshResult> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let result = Database::open(&db_path)
            .and_then(|database| indexer::reindex(&database, &sessions_dir))
            .map_err(|error| format!("{error:#}"));
        let _ = sender.send(result);
    });
    receiver
}

fn apply_refresh_results(
    database: &Database,
    state: &mut TuiState,
    refresh_status: &mut RefreshStatus,
    refresh_results: &Receiver<RefreshResult>,
) -> Result<()> {
    while let Ok(result) = refresh_results.try_recv() {
        match result {
            Ok(report) => {
                *refresh_status =
                    RefreshStatus::complete(report.scanned_files, report.indexed_sessions);
                state.reload(database)?;
            }
            Err(message) => {
                *refresh_status = RefreshStatus::failed(short_error_message(&message));
            }
        }
    }

    Ok(())
}

fn short_error_message(message: &str) -> String {
    trim_to_chars(message, 96)
}

fn handle_key(database: &Database, state: &mut TuiState, key_event: KeyEvent) -> Result<TuiAction> {
    match key_event.code {
        KeyCode::Esc => Ok(TuiAction::Quit),
        KeyCode::Enter => Ok(state
            .selected_session_id()
            .map(|session_id| TuiAction::Resume(session_id.to_string()))
            .unwrap_or(TuiAction::Continue)),
        KeyCode::Up => {
            state.move_up();
            Ok(TuiAction::Continue)
        }
        KeyCode::Down => {
            state.move_down();
            Ok(TuiAction::Continue)
        }
        KeyCode::PageDown => {
            state.scroll_preview_page_down();
            Ok(TuiAction::Continue)
        }
        KeyCode::PageUp => {
            state.scroll_preview_page_up();
            Ok(TuiAction::Continue)
        }
        KeyCode::Home => {
            state.scroll_preview_top();
            Ok(TuiAction::Continue)
        }
        KeyCode::Backspace => {
            let mut query = state.query().to_string();
            query.pop();
            state.set_query(database, query)?;
            Ok(TuiAction::Continue)
        }
        KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
            Ok(TuiAction::Quit)
        }
        KeyCode::Char('q') if state.query().is_empty() => Ok(TuiAction::Quit),
        KeyCode::Char('a') if state.query().is_empty() => {
            state.show_all_projects(database)?;
            Ok(TuiAction::Continue)
        }
        KeyCode::Char('p') if state.query().is_empty() => {
            state.show_current_directory(database)?;
            Ok(TuiAction::Continue)
        }
        KeyCode::Char('d') if state.query().is_empty() => {
            state.scroll_preview_down();
            Ok(TuiAction::Continue)
        }
        KeyCode::Char('u') if state.query().is_empty() => {
            state.scroll_preview_up();
            Ok(TuiAction::Continue)
        }
        KeyCode::Char('j') if state.query().is_empty() => {
            state.move_down();
            Ok(TuiAction::Continue)
        }
        KeyCode::Char('k') if state.query().is_empty() => {
            state.move_up();
            Ok(TuiAction::Continue)
        }
        KeyCode::Char(character) if key_event.modifiers.is_empty() => {
            let mut query = state.query().to_string();
            query.push(character);
            state.set_query(database, query)?;
            Ok(TuiAction::Continue)
        }
        _ => Ok(TuiAction::Continue),
    }
}

fn selected_preview(database: &Database, state: &TuiState) -> Result<Option<SessionDetail>> {
    let Some(session_id) = state.selected_session_id() else {
        return Ok(None);
    };
    database.get_session(session_id)
}

fn render(
    frame: &mut Frame<'_>,
    state: &TuiState,
    refresh_status: &RefreshStatus,
    preview: Option<&SessionDetail>,
) {
    let page_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_header(frame, page_chunks[0], state, refresh_status);
    render_search(frame, page_chunks[1], state);
    render_body(frame, page_chunks[2], state, refresh_status, preview);
    render_help(frame, page_chunks[3]);
}

fn render_header(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &TuiState,
    refresh_status: &RefreshStatus,
) {
    let header = Line::from(vec![
        Span::styled(
            "Codex Explorer",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(state.mode_label(), Style::default().fg(Color::Yellow)),
        Span::raw("  "),
        Span::styled(state.result_label(), secondary_style()),
        Span::raw("  "),
        Span::styled(refresh_status.label(), refresh_style(refresh_status)),
        Span::raw("  "),
        Span::styled(scope_note_label(state), secondary_style()),
    ]);
    frame.render_widget(Paragraph::new(header), area);
}

fn render_search(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let search_text = if state.query().is_empty() {
        Line::from(Span::styled(
            "Search sessions by task, repo, error, file...",
            secondary_style(),
        ))
    } else {
        Line::from(Span::styled(
            state.query().to_string(),
            Style::default().fg(Color::White),
        ))
    };
    let search = Paragraph::new(search_text).block(
        Block::default()
            .title(" Search ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(search, area);
}

fn render_body(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &TuiState,
    refresh_status: &RefreshStatus,
    preview: Option<&SessionDetail>,
) {
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(area);

    render_session_list(frame, body_chunks[0], state, refresh_status);
    render_preview(frame, body_chunks[1], state, refresh_status, preview);
}

fn render_session_list(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &TuiState,
    refresh_status: &RefreshStatus,
) {
    let items = if state.summaries().is_empty() {
        vec![ListItem::new(vec![Line::from(Span::styled(
            empty_state_message(state.query(), refresh_status),
            secondary_style(),
        ))])]
    } else {
        state
            .summaries()
            .iter()
            .map(session_list_item)
            .collect::<Vec<_>>()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Sessions ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">");
    let mut list_state = ListState::default();
    list_state.select(state.selected_index());

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn session_list_item(summary: &SessionSummary) -> ListItem<'static> {
    let title = Line::from(Span::styled(
        summary.title.clone(),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ));
    let metadata = Line::from(vec![
        Span::styled(compact_path(&summary.cwd), secondary_style()),
        Span::styled("  |  ", secondary_style()),
        Span::styled(compact_timestamp(&summary.started_at), secondary_style()),
    ]);

    ListItem::new(vec![title, metadata])
}

fn render_preview(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &TuiState,
    refresh_status: &RefreshStatus,
    preview: Option<&SessionDetail>,
) {
    let lines = match preview {
        Some(detail) => preview_lines(detail),
        None => vec![Line::from(Span::styled(
            empty_state_message(state.query(), refresh_status),
            secondary_style(),
        ))],
    };
    let preview = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Preview ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .scroll((state.preview_scroll() as u16, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(preview, area);
}

fn render_help(frame: &mut Frame<'_>, area: Rect) {
    let help = Paragraph::new(
        "Type search | a all | p project | Up/Down select | PgUp/PgDn preview | Enter resume | Esc quit",
    )
    .style(secondary_style());
    frame.render_widget(help, area);
}

fn scope_note_label(state: &TuiState) -> String {
    state.scope_note().unwrap_or("").to_string()
}

fn preview_lines(detail: &SessionDetail) -> Vec<Line<'static>> {
    let mut lines = vec![
        section_label("Task"),
        Line::from(Span::styled(
            detail.summary.title.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        section_label("Context"),
        Line::from(vec![
            Span::styled("Project: ", label_style()),
            Span::raw(compact_path(&detail.summary.cwd)),
        ]),
        Line::from(vec![
            Span::styled("Path: ", label_style()),
            Span::styled(detail.summary.cwd.clone(), secondary_style()),
        ]),
        Line::from(vec![
            Span::styled("Started: ", label_style()),
            Span::raw(compact_timestamp(&detail.summary.started_at)),
        ]),
        Line::from(""),
        section_label("Conversation"),
    ];

    let preview_messages = meaningful_preview_messages(&detail.messages, 8);
    if preview_messages.is_empty() {
        lines.push(Line::from(Span::styled(
            "No useful conversation text found",
            secondary_style(),
        )));
    }

    for message in preview_messages {
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", message.role), role_style(&message.role)),
            Span::raw(preview_text(&message.text)),
        ]));
    }

    lines.extend([
        Line::from(""),
        section_label("Resume"),
        Line::from(vec![
            Span::styled("Session: ", label_style()),
            Span::raw(short_session_id(&detail.summary.session_id)),
            Span::styled("  |  Enter to resume", Style::default().fg(Color::Cyan)),
        ]),
    ]);

    lines
}

fn section_label(label: &'static str) -> Line<'static> {
    Line::from(Span::styled(
        label,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))
}

fn preview_text(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    trim_to_chars(&normalized, 180)
}

fn trim_to_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let mut trimmed = text
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    trimmed.push_str("...");
    trimmed
}

fn label_style() -> Style {
    Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

fn secondary_style() -> Style {
    Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::DIM)
}

fn role_style(role: &str) -> Style {
    let color = if role == "user" {
        Color::Yellow
    } else {
        Color::Green
    };

    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn refresh_style(refresh_status: &RefreshStatus) -> Style {
    match refresh_status {
        RefreshStatus::Running { .. } => Style::default().fg(Color::Cyan),
        RefreshStatus::Complete { .. } => Style::default().fg(Color::Green),
        RefreshStatus::Failed { .. } => Style::default().fg(Color::Red),
    }
}

enum TuiAction {
    Continue,
    Quit,
    Resume(String),
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}
