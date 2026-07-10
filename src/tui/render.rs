use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::{
    audit::SessionAuditRecord,
    codex::{ParsedSessionItem, ParsedSessionItemKind},
    db::{SessionDetail, SessionSummary},
};

use super::{
    format::{
        agent_activity_text_lines, compact_path, compact_timestamp, conversation_windows,
        empty_state_message, group_tool_events, preview_text, session_overview_detail_rows,
        session_summary_text_lines, short_session_id, visible_tool_events,
    },
    refresh::RefreshStatus,
    state::{PreviewMode, TuiState},
    AuditRunStatus,
};

pub(super) fn render(
    frame: &mut Frame<'_>,
    state: &TuiState,
    refresh_status: &RefreshStatus,
    preview: Option<&SessionDetail>,
    audit: Option<&SessionAuditRecord>,
    audit_status: &AuditRunStatus,
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
    render_body(
        frame,
        page_chunks[2],
        state,
        refresh_status,
        preview,
        audit,
        audit_status,
    );
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
    audit: Option<&SessionAuditRecord>,
    audit_status: &AuditRunStatus,
) {
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(area);

    render_session_list(frame, body_chunks[0], state, refresh_status);
    render_preview(
        frame,
        body_chunks[1],
        state,
        refresh_status,
        preview,
        audit,
        audit_status,
    );
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
            .map(|summary| session_list_item(summary, state.is_summary_expanded(summary)))
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

fn session_list_item(summary: &SessionSummary, expanded: bool) -> ListItem<'static> {
    let lines = session_summary_text_lines(summary, expanded)
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            if index == 0 {
                Line::from(Span::styled(
                    line,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(line, secondary_style()))
            }
        })
        .collect::<Vec<_>>();
    ListItem::new(lines)
}

fn render_preview(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &TuiState,
    refresh_status: &RefreshStatus,
    preview: Option<&SessionDetail>,
    audit: Option<&SessionAuditRecord>,
    audit_status: &AuditRunStatus,
) {
    let lines = match preview {
        Some(detail) => preview_lines(state.preview_mode(), detail, audit, audit_status),
        None => vec![Line::from(Span::styled(
            empty_state_message(state.query(), refresh_status),
            secondary_style(),
        ))],
    };
    let preview = Paragraph::new(lines)
        .block(
            Block::default()
                .title(format!(" {} ", state.preview_mode_label()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .scroll((state.preview_scroll() as u16, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(preview, area);
}

fn render_help(frame: &mut Frame<'_>, area: Rect) {
    let help = Paragraph::new(
        "Type search | Alt+I audit | Alt+E expand | Alt+R reindex | Alt+1-5 previews | Alt+A all | Alt+P project | Enter resume | Esc quit",
    )
    .style(secondary_style());
    frame.render_widget(help, area);
}

fn scope_note_label(state: &TuiState) -> String {
    state.scope_note().unwrap_or("").to_string()
}

fn preview_lines(
    preview_mode: PreviewMode,
    detail: &SessionDetail,
    audit: Option<&SessionAuditRecord>,
    audit_status: &AuditRunStatus,
) -> Vec<Line<'static>> {
    match preview_mode {
        PreviewMode::Overview => overview_lines(detail),
        PreviewMode::Conversation => conversation_lines(detail),
        PreviewMode::Tools => tool_lines(detail),
        PreviewMode::Timeline => timeline_lines(detail),
        PreviewMode::Audit => audit_lines(detail, audit, audit_status),
    }
}

fn audit_lines(
    detail: &SessionDetail,
    audit: Option<&SessionAuditRecord>,
    audit_status: &AuditRunStatus,
) -> Vec<Line<'static>> {
    let mut lines = vec![section_label("Audit")];
    if let Some(status_label) = audit_status.label_for(&detail.summary.session_id) {
        lines.push(Line::from(Span::styled(status_label, secondary_style())));
        lines.push(Line::from(""));
    }

    let Some(audit) = audit else {
        lines.push(Line::from(Span::styled(
            "No cached audit for this session.",
            secondary_style(),
        )));
        lines.push(Line::from("Press i to run an audit."));
        return lines;
    };

    lines.push(Line::from(vec![
        Span::styled("Status: ", label_style()),
        Span::raw(audit.result.status.as_str().to_string()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Gist: ", label_style()),
        Span::raw(audit.result.gist.clone()),
    ]));
    lines.push(Line::from(""));
    lines.push(section_label("Hinge"));
    lines.push(Line::from(audit.result.hinge.clone()));
    lines.push(Line::from(""));
    lines.push(section_label("Next"));
    lines.push(Line::from(audit.result.next.clone()));
    lines.push(Line::from(""));
    lines.push(section_label("Signals"));
    for signal in &audit.result.signals {
        lines.push(Line::from(format!("- {signal}")));
    }
    lines.push(Line::from(""));
    lines.push(section_label("Run"));
    lines.push(Line::from(vec![
        Span::styled("Model: ", label_style()),
        Span::raw(audit.model.clone()),
        Span::styled("  Reasoning: ", label_style()),
        Span::raw(audit.reasoning_effort.clone()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Created: ", label_style()),
        Span::raw(audit.created_at.clone()),
    ]));
    lines
}

fn overview_lines(detail: &SessionDetail) -> Vec<Line<'static>> {
    let mut lines = vec![
        section_label("Started With"),
        Line::from(Span::styled(
            detail.summary.title.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        section_label("Most Recent User Message"),
        message_or_empty(latest_user_message(detail)),
        Line::from(""),
        section_label("Last Assistant Response"),
        message_or_empty(latest_assistant_message(detail)),
    ];
    append_agent_activity(&mut lines, detail);
    lines.push(Line::from(""));
    lines.push(section_label("Session Details"));
    append_overview_detail_rows(&mut lines, detail);

    add_action_lines(&mut lines, detail);
    lines
}

fn append_agent_activity(lines: &mut Vec<Line<'static>>, detail: &SessionDetail) {
    let activity_lines = agent_activity_text_lines(&detail.child_sessions);
    if activity_lines.is_empty() {
        return;
    }

    lines.push(Line::from(""));
    lines.push(section_label("Agent Activity"));
    lines.extend(activity_lines.into_iter().map(Line::from));
}

fn append_overview_detail_rows(lines: &mut Vec<Line<'static>>, detail: &SessionDetail) {
    for row in session_overview_detail_rows(&detail.summary) {
        let value_span = if row.label == "Path" {
            Span::styled(row.value, secondary_style())
        } else {
            Span::raw(row.value)
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{}: ", row.label), label_style()),
            value_span,
        ]));
    }
    lines.extend([
        Line::from(""),
        section_label("Counts"),
        Line::from(vec![
            Span::styled("Messages: ", label_style()),
            Span::raw(detail.messages.len().to_string()),
            Span::styled("  Tools: ", label_style()),
            Span::raw(detail.tool_events.len().to_string()),
            Span::styled("  Items: ", label_style()),
            Span::raw(detail.items.len().to_string()),
        ]),
    ]);
}

fn conversation_lines(detail: &SessionDetail) -> Vec<Line<'static>> {
    let windows = conversation_windows(&detail.messages, 5);
    let mut lines = vec![section_label("Opening Messages")];

    if windows.opening.is_empty() {
        lines.push(Line::from(Span::styled(
            "No useful conversation text found",
            secondary_style(),
        )));
    } else {
        append_messages(&mut lines, &windows.opening);
    }

    if windows.omitted_count > 0 {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("... {} messages omitted ...", windows.omitted_count),
            secondary_style(),
        )));
    }

    if !windows.recent.is_empty() {
        lines.push(Line::from(""));
        lines.push(section_label("Recent Messages"));
        append_messages(&mut lines, &windows.recent);
    }

    add_action_lines(&mut lines, detail);

    lines
}

fn tool_lines(detail: &SessionDetail) -> Vec<Line<'static>> {
    let groups = group_tool_events(&detail.tool_events);
    let mut lines = vec![
        section_label("Tool Summary"),
        Line::from(vec![
            Span::styled("Commands: ", label_style()),
            Span::raw(groups.commands.len().to_string()),
            Span::styled("  File changes: ", label_style()),
            Span::raw(groups.file_changes.len().to_string()),
            Span::styled("  Web: ", label_style()),
            Span::raw(groups.web_searches.len().to_string()),
            Span::styled("  Failures: ", label_style()),
            Span::raw(groups.failure_count.to_string()),
        ]),
        Line::from(""),
    ];

    if detail.tool_events.is_empty() {
        lines.push(Line::from(Span::styled(
            "No tool events found for this session",
            secondary_style(),
        )));
        return lines;
    }

    append_tool_section(&mut lines, "Failures", &groups.failures);
    append_tool_section(&mut lines, "Commands", &groups.commands);
    append_tool_section(&mut lines, "File Changes", &groups.file_changes);
    append_tool_section(&mut lines, "Web Searches", &groups.web_searches);
    append_tool_section(&mut lines, "Other Events", &groups.other_events);

    lines
}

fn timeline_lines(detail: &SessionDetail) -> Vec<Line<'static>> {
    let mut lines = vec![section_label("Session Timeline")];

    if detail.items.is_empty() {
        lines.push(Line::from(Span::styled(
            "No session items found for this session",
            secondary_style(),
        )));
        return lines;
    }

    for item in &detail.items {
        append_session_item(&mut lines, item);
    }

    lines
}

fn append_session_item(lines: &mut Vec<Line<'static>>, item: &ParsedSessionItem) {
    match &item.kind {
        ParsedSessionItemKind::SessionMeta(meta) => {
            lines.push(Line::from(vec![
                Span::styled(compact_timestamp(&item.timestamp), secondary_style()),
                Span::raw("  "),
                Span::styled("session_meta", label_style()),
            ]));
            if let Some(cwd) = &meta.cwd {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::raw(compact_path(cwd)),
                ]));
            }
        }
        ParsedSessionItemKind::Message(message) => {
            lines.push(Line::from(vec![
                Span::styled(compact_timestamp(&item.timestamp), secondary_style()),
                Span::raw("  "),
                Span::styled(message.role.clone(), role_style(&message.role)),
            ]));
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::raw(preview_text(&message.text)),
            ]));
        }
        ParsedSessionItemKind::ToolEvent(event) => {
            lines.push(Line::from(vec![
                Span::styled(compact_timestamp(&item.timestamp), secondary_style()),
                Span::raw("  "),
                Span::styled(event.name.clone(), label_style()),
                Span::styled(format!("  {}", event.kind), secondary_style()),
            ]));
            if !event.summary.trim().is_empty() {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::raw(preview_text(&event.summary)),
                ]));
            }
        }
        ParsedSessionItemKind::Unknown {
            record_type,
            payload_type,
            ..
        } => {
            let item_label = payload_type
                .as_ref()
                .map(|payload_type| format!("{record_type}/{payload_type}"))
                .unwrap_or_else(|| record_type.clone());
            lines.push(Line::from(vec![
                Span::styled(compact_timestamp(&item.timestamp), secondary_style()),
                Span::raw("  "),
                Span::styled(item_label, secondary_style()),
            ]));
        }
    }
}

fn append_tool_section(
    lines: &mut Vec<Line<'static>>,
    label: &'static str,
    events: &[&crate::codex::ParsedToolEvent],
) {
    if events.is_empty() {
        return;
    }

    lines.push(section_label(label));
    let visible = visible_tool_events(events);
    for event in visible.events {
        lines.push(Line::from(vec![
            Span::styled(compact_timestamp(&event.timestamp), secondary_style()),
            Span::raw("  "),
            Span::styled(event.name.clone(), label_style()),
        ]));
        if !event.summary.trim().is_empty() {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::raw(preview_text(&event.summary)),
            ]));
        }
        if let Some(status) = &event.status {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("status: {status}"), secondary_style()),
            ]));
        }
        let metadata = tool_event_metadata(event);
        if !metadata.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(metadata, secondary_style()),
            ]));
        }
    }
    if visible.omitted_count > 0 {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("{} more not shown", visible.omitted_count),
                secondary_style(),
            ),
        ]));
    }
    lines.push(Line::from(""));
}

fn tool_event_metadata(event: &crate::codex::ParsedToolEvent) -> String {
    let mut parts = Vec::new();
    if let Some(exit_code) = event.exit_code {
        parts.push(format!("exit: {exit_code}"));
    }
    if let Some(duration_ms) = event.duration_ms {
        parts.push(format!("duration: {}", compact_duration(duration_ms)));
    }
    if let Some(cwd) = &event.cwd {
        parts.push(format!("cwd: {}", compact_path(cwd)));
    }
    parts.join("  ")
}

fn compact_duration(duration_ms: i64) -> String {
    if duration_ms < 1000 {
        return format!("{duration_ms}ms");
    }

    let seconds = duration_ms as f64 / 1000.0;
    format!("{seconds:.1}s")
}

fn append_messages(lines: &mut Vec<Line<'static>>, messages: &[&crate::codex::ParsedMessage]) {
    for message in messages {
        lines.push(Line::from(vec![
            Span::styled(format!("{:<9}", message.role), role_style(&message.role)),
            Span::raw(preview_text(&message.text)),
        ]));
    }
}

fn message_or_empty(message: Option<&str>) -> Line<'static> {
    match message {
        Some(message) => Line::from(Span::raw(preview_text(message))),
        None => Line::from(Span::styled("No message found", secondary_style())),
    }
}

fn latest_user_message(detail: &SessionDetail) -> Option<&str> {
    detail
        .summary
        .latest_user_message
        .as_deref()
        .or_else(|| latest_message_by_role(&detail.messages, "user"))
}

fn latest_assistant_message(detail: &SessionDetail) -> Option<&str> {
    detail
        .summary
        .latest_assistant_message
        .as_deref()
        .or_else(|| latest_message_by_role(&detail.messages, "assistant"))
}

fn latest_message_by_role<'a>(
    messages: &'a [crate::codex::ParsedMessage],
    role: &str,
) -> Option<&'a str> {
    messages
        .iter()
        .rev()
        .find(|message| message.role == role)
        .map(|message| message.text.as_str())
}

fn add_action_lines(lines: &mut Vec<Line<'static>>, detail: &SessionDetail) {
    lines.extend([
        Line::from(""),
        section_label("Action"),
        Line::from(vec![
            Span::styled("Session: ", label_style()),
            Span::raw(short_session_id(&detail.summary.session_id)),
            Span::styled("  |  Enter to resume", Style::default().fg(Color::Cyan)),
        ]),
    ]);
}

fn section_label(label: &'static str) -> Line<'static> {
    Line::from(Span::styled(
        label.to_ascii_uppercase(),
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
