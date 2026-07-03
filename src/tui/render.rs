use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::db::{SessionDetail, SessionSummary};

use super::{
    format::{
        compact_path, compact_timestamp, empty_state_message, meaningful_preview_messages,
        preview_text, short_session_id,
    },
    refresh::RefreshStatus,
    state::TuiState,
};

pub(super) fn render(
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
