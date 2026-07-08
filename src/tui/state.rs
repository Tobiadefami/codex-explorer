use std::path::PathBuf;

use anyhow::Result;

use crate::db::{Database, SessionSummary};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectScope {
    CurrentDirectory,
    AllProjects,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewMode {
    Overview,
    Conversation,
    Tools,
    Timeline,
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
    preview_mode: PreviewMode,
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
            preview_mode: PreviewMode::Overview,
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
            preview_mode: PreviewMode::Overview,
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

    pub fn preview_mode(&self) -> PreviewMode {
        self.preview_mode
    }

    pub fn preview_mode_label(&self) -> &'static str {
        match self.preview_mode {
            PreviewMode::Overview => "Overview",
            PreviewMode::Conversation => "Conversation",
            PreviewMode::Tools => "Tools",
            PreviewMode::Timeline => "Timeline",
        }
    }

    pub fn set_preview_mode(&mut self, preview_mode: PreviewMode) {
        self.preview_mode = preview_mode;
        self.preview_scroll = 0;
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

fn first_index(summaries: &[SessionSummary]) -> Option<usize> {
    if summaries.is_empty() {
        None
    } else {
        Some(0)
    }
}
