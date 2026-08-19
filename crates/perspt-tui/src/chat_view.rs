//! Message-area and input-box rendering for the chat TUI (PSP-1
//! decomposition from `chat_app`). State stays in [`ChatApp`]; this module
//! owns the per-frame drawing of the wrapped message cache and the input.

use super::chat_app::{ChatApp, ChatMessage, ContentBlock};
use crate::theme::icons;
use ratatui::{
    layout::{Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};
use throbber_widgets_tui::Throbber;
use unicode_width::UnicodeWidthStr;

impl ChatApp {
    /// Render messages with markdown support and virtual scrolling
    pub(crate) fn render_messages(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(96, 125, 139)))
            .title(Span::styled(
                " Messages ",
                Style::default().fg(Color::Rgb(176, 190, 197)),
            ));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let viewport_width = inner.width as usize;
        let viewport_height = inner.height as usize;

        // Detect resize or first render
        let resize_detected =
            viewport_width != self.last_viewport_width || self.total_visual_lines == 0;
        self.last_viewport_width = viewport_width;
        self.visible_height = viewport_height;

        if resize_detected {
            for msg in &mut self.messages {
                msg.update_cache(viewport_width, self.show_reasoning);
            }
        }

        // Collect all pre-wrapped cached lines
        let mut visual_lines: Vec<Line<'static>> = Vec::new();
        for msg in &self.messages {
            for line in &msg.cached_visual_lines {
                visual_lines.push(line.clone());
            }
        }

        self.push_streaming_lines(&mut visual_lines, viewport_width);

        // Handle throbber when loading with empty buffers
        if self.is_streaming
            && self.streaming_buffer.is_empty()
            && self.streaming_reasoning.is_empty()
        {
            let throbber = Throbber::default()
                .label(" Thinking...")
                .style(Style::default().fg(Color::Rgb(255, 183, 77)));
            frame.render_stateful_widget(
                throbber,
                Rect::new(inner.x + 1, inner.y + 1, 20, 1),
                &mut self.throbber_state.clone(),
            );
        }

        // Calculate scroll position using visual line count
        let total_visual = visual_lines.len();
        self.total_visual_lines = total_visual;

        let max_scroll = total_visual.saturating_sub(viewport_height);

        let scroll_pos = if self.auto_scroll {
            max_scroll
        } else {
            self.scroll_offset.min(max_scroll)
        };

        // Update scroll_offset to actual position
        self.scroll_offset = scroll_pos;

        // Slice visible range and convert to Lines (virtual scrolling)
        let visible_lines: Vec<Line> = visual_lines
            .into_iter()
            .skip(scroll_pos)
            .take(viewport_height)
            .collect();

        // Render only the visible slice
        let paragraph = Paragraph::new(Text::from(visible_lines));
        frame.render_widget(paragraph, inner);

        // Scrollbar with accurate visual line count
        if total_visual > viewport_height {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .thumb_style(Style::default().fg(Color::Rgb(96, 125, 139)));
            let mut state = ScrollbarState::new(total_visual).position(scroll_pos);
            frame.render_stateful_widget(scrollbar, area.inner(Margin::new(0, 1)), &mut state);
        }
    }

    /// Append the streaming thought block (when reasoning is shown).
    fn push_thought_lines(
        visual_lines: &mut Vec<Line<'static>>,
        combined_thought: &Option<String>,
        viewport_width: usize,
    ) {
        if let Some(ref thought) = combined_thought {
            if !thought.is_empty() {
                visual_lines.push(Line::from(Span::styled(
                    "  ⚡ Thought Process".to_string(),
                    Style::default()
                        .fg(Color::Rgb(255, 183, 77))
                        .add_modifier(Modifier::ITALIC | Modifier::BOLD),
                )));
                let reasoning_style = Style::default()
                    .fg(Color::Rgb(120, 144, 156))
                    .add_modifier(Modifier::ITALIC);
                for line in thought.lines() {
                    let text = format!("    {}", line);
                    if text.width() <= viewport_width {
                        visual_lines.push(Line::from(Span::styled(text, reasoning_style)));
                    } else {
                        let wrapped = Self::wrap_text_to_width(&text, viewport_width);
                        for wrapped_line in wrapped {
                            visual_lines
                                .push(Line::from(Span::styled(wrapped_line, reasoning_style)));
                        }
                    }
                }
                visual_lines.push(Line::from(String::new()));
            }
        }
    }

    /// Append the in-flight streaming assistant content (header, optional
    /// thought block, markdown/math rendering, cursor) to the visual lines.
    fn push_streaming_lines(&self, visual_lines: &mut Vec<Line<'static>>, viewport_width: usize) {
        // Add streaming content on the fly
        if self.is_streaming
            && (!self.streaming_buffer.is_empty() || !self.streaming_reasoning.is_empty())
        {
            let header_style = Style::default()
                .fg(Color::Rgb(144, 202, 249))
                .add_modifier(Modifier::BOLD);
            let content_style = Style::default().fg(Color::Rgb(189, 189, 189));

            visual_lines.push(Line::from(Span::styled(
                format!("━━━ {} Assistant ━━━", icons::ASSISTANT),
                header_style,
            )));

            // Parse thoughts from streaming content
            let (inline_thought, display_content) =
                ChatMessage::parse_inline_thought(&self.streaming_buffer);
            let combined_thought = match (&self.streaming_reasoning, &inline_thought) {
                (r, Some(i)) if !r.is_empty() => Some(format!("{}\n{}", r, i)),
                (r, None) if !r.is_empty() => Some(r.clone()),
                (_, Some(i)) => Some(i.clone()),
                (_, None) => None,
            };

            if self.show_reasoning {
                Self::push_thought_lines(visual_lines, &combined_thought, viewport_width);
            }

            // Pre-transpile math segments in the streaming display content
            let display_content_transpiled =
                crate::markdown::transpile_math_in_text(&display_content);

            let blocks = crate::markdown::parse_markdown_blocks(&display_content_transpiled);
            for block in blocks {
                match block {
                    ContentBlock::Markdown(text) => {
                        let rendered = tui_markdown::from_str(&text);
                        for line in rendered.lines {
                            let text: String =
                                line.spans.iter().map(|s| s.content.as_ref()).collect();
                            let parsed_line =
                                crate::markdown::parse_line_to_spans(&text, content_style);
                            let wrapped = crate::markdown::wrap_line(parsed_line, viewport_width);
                            visual_lines.extend(wrapped);
                        }
                    }
                    ContentBlock::Table {
                        headers,
                        alignments,
                        rows,
                    } => {
                        let table_lines = crate::markdown::render_table(
                            headers,
                            alignments,
                            rows,
                            viewport_width,
                            content_style,
                        );
                        visual_lines.extend(table_lines);
                    }
                }
            }

            // Streaming cursor
            visual_lines.push(Line::from(Span::styled(
                "▌".to_string(),
                Style::default()
                    .fg(Color::Rgb(129, 212, 250))
                    .add_modifier(Modifier::SLOW_BLINK),
            )));
        }
    }

    pub(crate) fn render_input(&self, frame: &mut Frame, area: Rect) {
        if self.is_streaming {
            // Show streaming indicator
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(96, 125, 139)))
                .title(Span::styled(
                    " Receiving response... ",
                    Style::default().fg(Color::Rgb(255, 183, 77)),
                ));
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let text = Paragraph::new("Press Ctrl+C to cancel")
                .style(Style::default().fg(Color::Rgb(120, 144, 156)));
            frame.render_widget(text, inner);
        } else {
            // Render input with hint
            self.input
                .render(frame, area, "Enter=send │ Ctrl+J=newline");
        }
    }
}
