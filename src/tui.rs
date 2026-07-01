use std::io;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::db::{Database, SessionDetail, SessionSummary};

const TUI_RESULT_LIMIT: usize = 50;

pub struct TuiState {
    query: String,
    summaries: Vec<SessionSummary>,
    selected_index: Option<usize>,
    limit: usize,
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
        })
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn summaries(&self) -> &[SessionSummary] {
        &self.summaries
    }

    pub fn set_query(&mut self, database: &Database, query: String) -> Result<()> {
        self.query = query;
        self.summaries = if self.query.trim().is_empty() {
            database.list_sessions(self.limit)?
        } else {
            database.search_sessions(&self.query, self.limit)?
        };
        self.selected_index = first_index(&self.summaries);
        Ok(())
    }

    pub fn move_down(&mut self) {
        let Some(selected_index) = self.selected_index else {
            return;
        };
        let last_index = self.summaries.len().saturating_sub(1);
        self.selected_index = Some((selected_index + 1).min(last_index));
    }

    pub fn move_up(&mut self) {
        let Some(selected_index) = self.selected_index else {
            return;
        };
        self.selected_index = Some(selected_index.saturating_sub(1));
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
}

fn first_index(summaries: &[SessionSummary]) -> Option<usize> {
    if summaries.is_empty() {
        None
    } else {
        Some(0)
    }
}

pub fn run(database: &Database) -> Result<Option<String>> {
    let _terminal_guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut state = TuiState::load(database, TUI_RESULT_LIMIT)?;

    loop {
        let preview = selected_preview(database, &state)?;
        terminal.draw(|frame| render(frame, &state, preview.as_ref()))?;

        if let Event::Key(key_event) = event::read()? {
            match handle_key(database, &mut state, key_event)? {
                TuiAction::Continue => {}
                TuiAction::Quit => return Ok(None),
                TuiAction::Resume(session_id) => return Ok(Some(session_id)),
            }
        }
    }
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

fn render(frame: &mut Frame<'_>, state: &TuiState, preview: Option<&SessionDetail>) {
    let page_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(frame.area());

    render_search(frame, page_chunks[0], state);
    render_body(frame, page_chunks[1], state, preview);
    render_help(frame, page_chunks[2]);
}

fn render_search(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let search =
        Paragraph::new(state.query()).block(Block::default().title("Search").borders(Borders::ALL));
    frame.render_widget(search, area);
}

fn render_body(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &TuiState,
    preview: Option<&SessionDetail>,
) {
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    render_session_list(frame, body_chunks[0], state);
    render_preview(frame, body_chunks[1], preview);
}

fn render_session_list(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let items = state
        .summaries()
        .iter()
        .map(|summary| {
            let title = format!("{}  {}", summary.started_at, summary.title);
            let cwd = format!("  {}", summary.cwd);
            ListItem::new(vec![Line::from(title), Line::from(cwd)])
        })
        .collect::<Vec<_>>();

    let list = List::new(items)
        .block(Block::default().title("Sessions").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    let mut list_state = ListState::default();
    list_state.select(state.selected_index());

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_preview(frame: &mut Frame<'_>, area: Rect, preview: Option<&SessionDetail>) {
    let lines = match preview {
        Some(detail) => preview_lines(detail),
        None => vec![Line::from("No session selected")],
    };
    let preview = Paragraph::new(lines)
        .block(Block::default().title("Preview").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(preview, area);
}

fn render_help(frame: &mut Frame<'_>, area: Rect) {
    let help = Paragraph::new(
        "Type to search | Backspace edits | Up/Down move | Enter resumes | Esc quits",
    )
    .style(Style::default().add_modifier(Modifier::DIM));
    frame.render_widget(help, area);
}

fn preview_lines(detail: &SessionDetail) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("id: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(detail.summary.session_id.clone()),
        ]),
        Line::from(vec![
            Span::styled("cwd: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(detail.summary.cwd.clone()),
        ]),
        Line::from(vec![
            Span::styled("started: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(detail.summary.started_at.clone()),
        ]),
        Line::from(""),
    ];

    for message in detail.messages.iter().take(12) {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{}: ", message.role),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(message.text.replace('\n', " ")),
        ]));
    }

    lines
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
