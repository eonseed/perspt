//! LLM Logs Viewer TUI
//!
//! A beautiful terminal interface for analyzing LLM request/response logs
//! from stored sessions.

use crate::theme::{icons, Theme};
use perspt_store::{LlmRequestRecord, SessionRecord, SessionStore};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Tabs, Wrap,
    },
    Frame,
};

/// Active panel in the logs viewer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePanel {
    Sessions,
    Requests,
    Detail,
}

/// Detail view tab
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    Prompt,
    Response,
    Stats,
}

/// Logs viewer application state
pub struct LogsViewer {
    /// Theme for styling
    theme: Theme,
    /// Session store
    store: SessionStore,
    /// List of sessions
    sessions: Vec<SessionRecord>,
    /// LLM requests for selected session
    requests: Vec<LlmRequestRecord>,
    /// Currently active panel
    active_panel: ActivePanel,
    /// Session list state
    session_state: ListState,
    /// Request list state
    request_state: ListState,
    /// Detail scroll position
    detail_scroll: u16,
    /// Detail view tab
    detail_tab: DetailTab,
    /// Should quit flag
    pub should_quit: bool,
    /// Show help modal
    show_help: bool,
}

impl LogsViewer {
    /// Create a new logs viewer
    pub fn new(store: SessionStore) -> Self {
        let sessions = store.list_recent_sessions(100).unwrap_or_default();
        let mut session_state = ListState::default();
        if !sessions.is_empty() {
            session_state.select(Some(0));
        }

        let mut viewer = Self {
            theme: Theme::dark(),
            store,
            sessions,
            requests: Vec::new(),
            active_panel: ActivePanel::Sessions,
            session_state,
            request_state: ListState::default(),
            detail_scroll: 0,
            detail_tab: DetailTab::Prompt,
            should_quit: false,
            show_help: false,
        };

        // Load requests for first session if available
        viewer.load_requests_for_selected_session();
        viewer
    }

    /// Load LLM requests for the currently selected session
    fn load_requests_for_selected_session(&mut self) {
        if let Some(idx) = self.session_state.selected() {
            if let Some(session) = self.sessions.get(idx) {
                self.requests = self
                    .store
                    .get_llm_requests(&session.session_id)
                    .unwrap_or_default();
                self.request_state = ListState::default();
                if !self.requests.is_empty() {
                    self.request_state.select(Some(0));
                }
                self.detail_scroll = 0;
            }
        }
    }

    /// Handle keyboard input
    pub fn handle_key(&mut self, key: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;

        if self.show_help {
            self.show_help = false;
            return;
        }

        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('?') | KeyCode::F(1) => self.show_help = true,
            KeyCode::Tab => self.next_panel(),
            KeyCode::BackTab => self.prev_panel(),
            KeyCode::Char('1') => self.detail_tab = DetailTab::Prompt,
            KeyCode::Char('2') => self.detail_tab = DetailTab::Response,
            KeyCode::Char('3') => self.detail_tab = DetailTab::Stats,
            KeyCode::Up | KeyCode::Char('k') => self.move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_down(),
            KeyCode::PageUp => self.page_up(),
            KeyCode::PageDown => self.page_down(),
            KeyCode::Home => self.move_to_start(),
            KeyCode::End => self.move_to_end(),
            KeyCode::Enter => self.select_item(),
            KeyCode::Left | KeyCode::Char('h') => {
                if self.active_panel == ActivePanel::Detail && self.detail_scroll > 0 {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                } else {
                    self.prev_panel();
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.active_panel == ActivePanel::Detail {
                    self.detail_scroll = self.detail_scroll.saturating_add(1);
                } else {
                    self.next_panel();
                }
            }
            _ => {}
        }
    }

    fn next_panel(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::Sessions => ActivePanel::Requests,
            ActivePanel::Requests => ActivePanel::Detail,
            ActivePanel::Detail => ActivePanel::Sessions,
        };
    }

    fn prev_panel(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::Sessions => ActivePanel::Detail,
            ActivePanel::Requests => ActivePanel::Sessions,
            ActivePanel::Detail => ActivePanel::Requests,
        };
    }

    fn move_up(&mut self) {
        match self.active_panel {
            ActivePanel::Sessions => {
                if let Some(idx) = self.session_state.selected() {
                    if idx > 0 {
                        self.session_state.select(Some(idx - 1));
                        self.load_requests_for_selected_session();
                    }
                }
            }
            ActivePanel::Requests => {
                if let Some(idx) = self.request_state.selected() {
                    if idx > 0 {
                        self.request_state.select(Some(idx - 1));
                        self.detail_scroll = 0;
                    }
                }
            }
            ActivePanel::Detail => {
                self.detail_scroll = self.detail_scroll.saturating_sub(3);
            }
        }
    }

    fn move_down(&mut self) {
        match self.active_panel {
            ActivePanel::Sessions => {
                if let Some(idx) = self.session_state.selected() {
                    if idx < self.sessions.len().saturating_sub(1) {
                        self.session_state.select(Some(idx + 1));
                        self.load_requests_for_selected_session();
                    }
                }
            }
            ActivePanel::Requests => {
                if let Some(idx) = self.request_state.selected() {
                    if idx < self.requests.len().saturating_sub(1) {
                        self.request_state.select(Some(idx + 1));
                        self.detail_scroll = 0;
                    }
                }
            }
            ActivePanel::Detail => {
                self.detail_scroll = self.detail_scroll.saturating_add(3);
            }
        }
    }

    fn page_up(&mut self) {
        match self.active_panel {
            ActivePanel::Sessions => {
                if let Some(idx) = self.session_state.selected() {
                    let new_idx = idx.saturating_sub(10);
                    self.session_state.select(Some(new_idx));
                    self.load_requests_for_selected_session();
                }
            }
            ActivePanel::Requests => {
                if let Some(idx) = self.request_state.selected() {
                    let new_idx = idx.saturating_sub(10);
                    self.request_state.select(Some(new_idx));
                    self.detail_scroll = 0;
                }
            }
            ActivePanel::Detail => {
                self.detail_scroll = self.detail_scroll.saturating_sub(20);
            }
        }
    }

    fn page_down(&mut self) {
        match self.active_panel {
            ActivePanel::Sessions => {
                if let Some(idx) = self.session_state.selected() {
                    let new_idx = (idx + 10).min(self.sessions.len().saturating_sub(1));
                    self.session_state.select(Some(new_idx));
                    self.load_requests_for_selected_session();
                }
            }
            ActivePanel::Requests => {
                if let Some(idx) = self.request_state.selected() {
                    let new_idx = (idx + 10).min(self.requests.len().saturating_sub(1));
                    self.request_state.select(Some(new_idx));
                    self.detail_scroll = 0;
                }
            }
            ActivePanel::Detail => {
                self.detail_scroll = self.detail_scroll.saturating_add(20);
            }
        }
    }

    fn move_to_start(&mut self) {
        match self.active_panel {
            ActivePanel::Sessions => {
                if !self.sessions.is_empty() {
                    self.session_state.select(Some(0));
                    self.load_requests_for_selected_session();
                }
            }
            ActivePanel::Requests => {
                if !self.requests.is_empty() {
                    self.request_state.select(Some(0));
                    self.detail_scroll = 0;
                }
            }
            ActivePanel::Detail => {
                self.detail_scroll = 0;
            }
        }
    }

    fn move_to_end(&mut self) {
        match self.active_panel {
            ActivePanel::Sessions => {
                if !self.sessions.is_empty() {
                    self.session_state.select(Some(self.sessions.len() - 1));
                    self.load_requests_for_selected_session();
                }
            }
            ActivePanel::Requests => {
                if !self.requests.is_empty() {
                    self.request_state.select(Some(self.requests.len() - 1));
                    self.detail_scroll = 0;
                }
            }
            ActivePanel::Detail => {
                // Will be clamped by render
                self.detail_scroll = u16::MAX;
            }
        }
    }

    fn select_item(&mut self) {
        match self.active_panel {
            ActivePanel::Sessions => {
                self.active_panel = ActivePanel::Requests;
            }
            ActivePanel::Requests => {
                self.active_panel = ActivePanel::Detail;
            }
            ActivePanel::Detail => {}
        }
    }

    /// Render the logs viewer
    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        // Main layout: header, content, footer
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(10),   // Content
                Constraint::Length(2), // Footer
            ])
            .split(area);

        self.render_header(frame, main_chunks[0]);
        self.render_content(frame, main_chunks[1]);
        self.render_footer(frame, main_chunks[2]);

        // Render help modal if active
        if self.show_help {
            self.render_help_modal(frame, area);
        }
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let title = format!(
            " {} LLM Logs Analyzer │ {} sessions │ {} requests ",
            icons::ASSISTANT,
            self.sessions.len(),
            self.requests.len()
        );

        let header = Paragraph::new(title)
            .style(
                Style::default()
                    .fg(self.theme.palette.primary)
                    .add_modifier(Modifier::BOLD),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.theme.palette.border))
                    .border_type(ratatui::widgets::BorderType::Rounded),
            );

        frame.render_widget(header, area);
    }

    fn render_content(&mut self, frame: &mut Frame, area: Rect) {
        // Three-panel layout
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25), // Sessions
                Constraint::Percentage(30), // Requests
                Constraint::Percentage(45), // Detail
            ])
            .split(area);

        self.render_sessions_panel(frame, chunks[0]);
        self.render_requests_panel(frame, chunks[1]);
        self.render_detail_panel(frame, chunks[2]);
    }

    fn render_sessions_panel(&mut self, frame: &mut Frame, area: Rect) {
        let is_active = self.active_panel == ActivePanel::Sessions;
        let border_style = if is_active {
            Style::default().fg(self.theme.palette.primary)
        } else {
            Style::default().fg(self.theme.palette.border)
        };

        let items: Vec<ListItem> = self
            .sessions
            .iter()
            .map(|session| {
                let status_icon = match session.status.as_str() {
                    "COMPLETED" => icons::SUCCESS,
                    "RUNNING" => icons::RUNNING,
                    "FAILED" => icons::FAILURE,
                    _ => icons::PENDING,
                };

                let task_preview = if session.task.len() > 20 {
                    format!("{}...", &session.task[..17])
                } else {
                    session.task.clone()
                };

                let line = Line::from(vec![
                    Span::styled(
                        format!("{} ", status_icon),
                        self.theme.status_style(&session.status),
                    ),
                    Span::styled(
                        task_preview,
                        Style::default().fg(self.theme.palette.on_surface),
                    ),
                ]);

                ListItem::new(line)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(format!(" {} Sessions ", icons::FOLDER))
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .border_type(ratatui::widgets::BorderType::Rounded),
            )
            .highlight_style(self.theme.highlight)
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, area, &mut self.session_state);

        // Scrollbar
        if self.sessions.len() > area.height as usize - 2 {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));
            let mut scrollbar_state = ScrollbarState::new(self.sessions.len())
                .position(self.session_state.selected().unwrap_or(0));
            frame.render_stateful_widget(
                scrollbar,
                area.inner(ratatui::layout::Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut scrollbar_state,
            );
        }
    }

    fn render_requests_panel(&mut self, frame: &mut Frame, area: Rect) {
        let is_active = self.active_panel == ActivePanel::Requests;
        let border_style = if is_active {
            Style::default().fg(self.theme.palette.primary)
        } else {
            Style::default().fg(self.theme.palette.border)
        };

        let items: Vec<ListItem> = self
            .requests
            .iter()
            .enumerate()
            .map(|(idx, req)| {
                let model_short = if req.model.len() > 15 {
                    format!("{}...", &req.model[..12])
                } else {
                    req.model.clone()
                };

                let lines = vec![
                    Line::from(vec![
                        Span::styled(
                            format!("#{} ", idx + 1),
                            Style::default()
                                .fg(self.theme.palette.secondary)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            model_short,
                            Style::default().fg(self.theme.palette.on_surface),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            format!("   {}ms ", req.latency_ms),
                            Style::default().fg(self.theme.palette.on_surface_muted),
                        ),
                        Span::styled(
                            format!("{}→{} chars", req.prompt.len(), req.response.len()),
                            Style::default().fg(self.theme.palette.on_surface_muted),
                        ),
                    ]),
                ];

                ListItem::new(lines)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(format!(" {} Requests ", icons::ASSISTANT))
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .border_type(ratatui::widgets::BorderType::Rounded),
            )
            .highlight_style(self.theme.highlight)
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, area, &mut self.request_state);

        // Scrollbar
        if self.requests.len() > (area.height as usize - 2) / 2 {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));
            let mut scrollbar_state = ScrollbarState::new(self.requests.len())
                .position(self.request_state.selected().unwrap_or(0));
            frame.render_stateful_widget(
                scrollbar,
                area.inner(ratatui::layout::Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut scrollbar_state,
            );
        }
    }

    fn render_detail_panel(&mut self, frame: &mut Frame, area: Rect) {
        let is_active = self.active_panel == ActivePanel::Detail;
        let border_style = if is_active {
            Style::default().fg(self.theme.palette.primary)
        } else {
            Style::default().fg(self.theme.palette.border)
        };

        // Tab header
        let tabs_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 3,
        };

        let content_area = Rect {
            x: area.x,
            y: area.y + 3,
            width: area.width,
            height: area.height.saturating_sub(3),
        };

        // Render tabs
        let tab_titles = vec!["[1] Prompt", "[2] Response", "[3] Stats"];
        let selected_tab = match self.detail_tab {
            DetailTab::Prompt => 0,
            DetailTab::Response => 1,
            DetailTab::Stats => 2,
        };

        let tabs = Tabs::new(tab_titles)
            .block(
                Block::default()
                    .title(format!(" {} Detail View ", icons::FILE))
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .border_type(ratatui::widgets::BorderType::Rounded),
            )
            .select(selected_tab)
            .style(Style::default().fg(self.theme.palette.on_surface_muted))
            .highlight_style(
                Style::default()
                    .fg(self.theme.palette.primary)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(tabs, tabs_area);

        // Render content based on selected tab
        // Clone data to avoid borrow checker issues
        let request_data = self.request_state.selected().and_then(|idx| {
            self.requests
                .get(idx)
                .map(|r| (r.prompt.clone(), r.response.clone(), r.clone()))
        });

        if let Some((prompt, response, request)) = request_data {
            match self.detail_tab {
                DetailTab::Prompt => {
                    self.render_text_content(frame, content_area, &prompt, "Prompt");
                }
                DetailTab::Response => {
                    self.render_text_content(frame, content_area, &response, "Response");
                }
                DetailTab::Stats => {
                    self.render_stats(frame, content_area, &request);
                }
            }
        } else {
            self.render_empty_detail(frame, content_area);
        }
    }

    fn render_text_content(&mut self, frame: &mut Frame, area: Rect, text: &str, _title: &str) {
        let lines: Vec<Line> = text
            .lines()
            .enumerate()
            .map(|(idx, line)| {
                Line::from(vec![
                    Span::styled(
                        format!("{:4} │ ", idx + 1),
                        Style::default().fg(self.theme.palette.on_surface_muted),
                    ),
                    Span::styled(
                        line.to_string(),
                        Style::default().fg(self.theme.palette.on_surface),
                    ),
                ])
            })
            .collect();

        let total_lines = lines.len() as u16;
        let visible_height = area.height.saturating_sub(2);

        // Clamp scroll
        if total_lines > visible_height {
            self.detail_scroll = self
                .detail_scroll
                .min(total_lines.saturating_sub(visible_height));
        } else {
            self.detail_scroll = 0;
        }

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                    .border_style(Style::default().fg(self.theme.palette.border))
                    .border_type(ratatui::widgets::BorderType::Rounded),
            )
            .scroll((self.detail_scroll, 0))
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);

        // Scrollbar
        if total_lines > visible_height {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));
            let mut scrollbar_state =
                ScrollbarState::new(total_lines as usize).position(self.detail_scroll as usize);
            frame.render_stateful_widget(
                scrollbar,
                area.inner(ratatui::layout::Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut scrollbar_state,
            );
        }
    }

    fn render_stats(&self, frame: &mut Frame, area: Rect, request: &LlmRequestRecord) {
        let stats_text = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  Model:      ",
                    Style::default().fg(self.theme.palette.secondary),
                ),
                Span::styled(
                    &request.model,
                    Style::default().fg(self.theme.palette.on_surface),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  Latency:    ",
                    Style::default().fg(self.theme.palette.secondary),
                ),
                Span::styled(
                    format!("{}ms", request.latency_ms),
                    self.latency_style(request.latency_ms),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  Prompt:     ",
                    Style::default().fg(self.theme.palette.secondary),
                ),
                Span::styled(
                    format!("{} characters", request.prompt.len()),
                    Style::default().fg(self.theme.palette.on_surface),
                ),
            ]),
            Line::from(vec![
                Span::styled("              ", Style::default()),
                Span::styled(
                    format!("{} lines", request.prompt.lines().count()),
                    Style::default().fg(self.theme.palette.on_surface_muted),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  Response:   ",
                    Style::default().fg(self.theme.palette.secondary),
                ),
                Span::styled(
                    format!("{} characters", request.response.len()),
                    Style::default().fg(self.theme.palette.on_surface),
                ),
            ]),
            Line::from(vec![
                Span::styled("              ", Style::default()),
                Span::styled(
                    format!("{} lines", request.response.lines().count()),
                    Style::default().fg(self.theme.palette.on_surface_muted),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  Tokens In:  ",
                    Style::default().fg(self.theme.palette.secondary),
                ),
                Span::styled(
                    format!("{}", request.tokens_in),
                    Style::default().fg(self.theme.palette.on_surface),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "  Tokens Out: ",
                    Style::default().fg(self.theme.palette.secondary),
                ),
                Span::styled(
                    format!("{}", request.tokens_out),
                    Style::default().fg(self.theme.palette.on_surface),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  Node ID:    ",
                    Style::default().fg(self.theme.palette.secondary),
                ),
                Span::styled(
                    request.node_id.as_deref().unwrap_or("(none)"),
                    Style::default().fg(self.theme.palette.on_surface_muted),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  Session:    ",
                    Style::default().fg(self.theme.palette.secondary),
                ),
                Span::styled(
                    &request.session_id,
                    Style::default().fg(self.theme.palette.on_surface_muted),
                ),
            ]),
        ];

        let paragraph = Paragraph::new(stats_text).block(
            Block::default()
                .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                .border_style(Style::default().fg(self.theme.palette.border))
                .border_type(ratatui::widgets::BorderType::Rounded),
        );

        frame.render_widget(paragraph, area);
    }

    fn latency_style(&self, latency_ms: i32) -> Style {
        if latency_ms < 1000 {
            self.theme.success
        } else if latency_ms < 3000 {
            self.theme.warning
        } else {
            self.theme.error
        }
    }

    fn render_empty_detail(&self, frame: &mut Frame, area: Rect) {
        let text = vec![
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "       No request selected",
                Style::default().fg(self.theme.palette.on_surface_muted),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "    Select a session and request",
                Style::default().fg(self.theme.palette.on_surface_muted),
            )),
            Line::from(Span::styled(
                "       to view details",
                Style::default().fg(self.theme.palette.on_surface_muted),
            )),
        ];

        let paragraph = Paragraph::new(text).block(
            Block::default()
                .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                .border_style(Style::default().fg(self.theme.palette.border))
                .border_type(ratatui::widgets::BorderType::Rounded),
        );

        frame.render_widget(paragraph, area);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let help_text = Line::from(vec![
            Span::styled(" q", Style::default().fg(self.theme.palette.primary)),
            Span::styled(
                " quit ",
                Style::default().fg(self.theme.palette.on_surface_muted),
            ),
            Span::styled("│", Style::default().fg(self.theme.palette.border)),
            Span::styled(" Tab", Style::default().fg(self.theme.palette.primary)),
            Span::styled(
                " switch panel ",
                Style::default().fg(self.theme.palette.on_surface_muted),
            ),
            Span::styled("│", Style::default().fg(self.theme.palette.border)),
            Span::styled(" ↑↓/jk", Style::default().fg(self.theme.palette.primary)),
            Span::styled(
                " navigate ",
                Style::default().fg(self.theme.palette.on_surface_muted),
            ),
            Span::styled("│", Style::default().fg(self.theme.palette.border)),
            Span::styled(" 1/2/3", Style::default().fg(self.theme.palette.primary)),
            Span::styled(
                " tabs ",
                Style::default().fg(self.theme.palette.on_surface_muted),
            ),
            Span::styled("│", Style::default().fg(self.theme.palette.border)),
            Span::styled(" ?", Style::default().fg(self.theme.palette.primary)),
            Span::styled(
                " help ",
                Style::default().fg(self.theme.palette.on_surface_muted),
            ),
        ]);

        let footer = Paragraph::new(help_text)
            .style(Style::default())
            .alignment(ratatui::layout::Alignment::Center);

        frame.render_widget(footer, area);
    }

    fn render_help_modal(&self, frame: &mut Frame, area: Rect) {
        // Calculate modal size
        let modal_width = 60.min(area.width.saturating_sub(4));
        let modal_height = 20.min(area.height.saturating_sub(4));

        let modal_area = Rect {
            x: (area.width - modal_width) / 2,
            y: (area.height - modal_height) / 2,
            width: modal_width,
            height: modal_height,
        };

        // Clear background
        frame.render_widget(Clear, modal_area);

        let help_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Navigation",
                Style::default()
                    .fg(self.theme.palette.primary)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("  Tab / Shift+Tab    Switch panels"),
            Line::from("  ↑/↓ or j/k         Navigate items"),
            Line::from("  PgUp / PgDn        Page navigation"),
            Line::from("  Home / End         Jump to start/end"),
            Line::from("  Enter              Select item"),
            Line::from(""),
            Line::from(Span::styled(
                "  Detail View",
                Style::default()
                    .fg(self.theme.palette.primary)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("  1                  Show prompt"),
            Line::from("  2                  Show response"),
            Line::from("  3                  Show stats"),
            Line::from("  ←/→ or h/l         Scroll content"),
            Line::from(""),
            Line::from(Span::styled(
                "  Press any key to close",
                Style::default().fg(self.theme.palette.on_surface_muted),
            )),
        ];

        let help = Paragraph::new(help_text)
            .block(
                Block::default()
                    .title(" ⌨ Keyboard Shortcuts ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.theme.palette.primary))
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .style(Style::default().bg(Color::Rgb(30, 30, 40))),
            )
            .style(Style::default().fg(self.theme.palette.on_surface));

        frame.render_widget(help, modal_area);
    }
}

/// Run the logs viewer TUI
pub async fn run_logs_viewer() -> anyhow::Result<()> {
    use anyhow::Context;
    use crossterm::event::{self, Event, KeyEventKind};

    let store = SessionStore::new().context("Failed to open session store")?;
    let mut app = LogsViewer::new(store);

    let mut terminal = ratatui::init();

    loop {
        terminal.draw(|frame| app.render(frame))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key.code);
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    ratatui::restore();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panel_navigation() {
        // Test panel cycling
        let mut panel = ActivePanel::Sessions;

        // Forward
        panel = match panel {
            ActivePanel::Sessions => ActivePanel::Requests,
            ActivePanel::Requests => ActivePanel::Detail,
            ActivePanel::Detail => ActivePanel::Sessions,
        };
        assert_eq!(panel, ActivePanel::Requests);

        panel = match panel {
            ActivePanel::Sessions => ActivePanel::Requests,
            ActivePanel::Requests => ActivePanel::Detail,
            ActivePanel::Detail => ActivePanel::Sessions,
        };
        assert_eq!(panel, ActivePanel::Detail);

        panel = match panel {
            ActivePanel::Sessions => ActivePanel::Requests,
            ActivePanel::Requests => ActivePanel::Detail,
            ActivePanel::Detail => ActivePanel::Sessions,
        };
        assert_eq!(panel, ActivePanel::Sessions);
    }

    #[test]
    fn test_detail_tab_selection() {
        let tab = DetailTab::Prompt;
        assert_eq!(tab, DetailTab::Prompt);

        let tab = DetailTab::Response;
        assert_eq!(tab, DetailTab::Response);

        let tab = DetailTab::Stats;
        assert_eq!(tab, DetailTab::Stats);
    }
}
