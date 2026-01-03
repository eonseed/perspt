//! Simple Input Widget for Chat
//!
//! A minimal, reliable input component that gives us full control
//! over key handling without fighting with tui-textarea's internal state.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// A simple multi-line text input with reliable key handling
#[derive(Debug, Clone)]
pub struct SimpleInput {
    /// Lines of text
    lines: Vec<String>,
    /// Current cursor line (0-indexed)
    cursor_line: usize,
    /// Current cursor column (0-indexed)
    cursor_col: usize,
    /// Whether the input is focused
    focused: bool,
}

impl Default for SimpleInput {
    fn default() -> Self {
        Self::new()
    }
}

impl SimpleInput {
    /// Create a new empty input
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            focused: true,
        }
    }

    /// Insert a character at the cursor position
    pub fn insert_char(&mut self, c: char) {
        if self.cursor_line < self.lines.len() {
            let line = &mut self.lines[self.cursor_line];
            // Clamp cursor_col to line length
            self.cursor_col = self.cursor_col.min(line.len());
            line.insert(self.cursor_col, c);
            self.cursor_col += 1;
        }
    }

    /// Insert a newline at the cursor position
    pub fn insert_newline(&mut self) {
        if self.cursor_line < self.lines.len() {
            let line = &mut self.lines[self.cursor_line];
            self.cursor_col = self.cursor_col.min(line.len());

            // Split the current line at cursor
            let remainder = line[self.cursor_col..].to_string();
            line.truncate(self.cursor_col);

            // Insert new line after current
            self.cursor_line += 1;
            self.lines.insert(self.cursor_line, remainder);
            self.cursor_col = 0;
        }
    }

    /// Delete character before cursor (backspace)
    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            // Delete within line
            let line = &mut self.lines[self.cursor_line];
            self.cursor_col -= 1;
            line.remove(self.cursor_col);
        } else if self.cursor_line > 0 {
            // Merge with previous line
            let current_line = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            self.cursor_col = self.lines[self.cursor_line].len();
            self.lines[self.cursor_line].push_str(&current_line);
        }
    }

    /// Delete character at cursor (delete key)
    pub fn delete(&mut self) {
        let line = &mut self.lines[self.cursor_line];
        if self.cursor_col < line.len() {
            line.remove(self.cursor_col);
        } else if self.cursor_line + 1 < self.lines.len() {
            // Merge next line into current
            let next_line = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].push_str(&next_line);
        }
    }

    /// Move cursor left
    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.lines[self.cursor_line].len();
        }
    }

    /// Move cursor right
    pub fn move_right(&mut self) {
        let line_len = self.lines[self.cursor_line].len();
        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    /// Move cursor up
    pub fn move_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.cursor_col.min(self.lines[self.cursor_line].len());
        }
    }

    /// Move cursor down
    pub fn move_down(&mut self) {
        if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.cursor_col = self.cursor_col.min(self.lines[self.cursor_line].len());
        }
    }

    /// Move cursor to start of line
    pub fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    /// Move cursor to end of line
    pub fn move_end(&mut self) {
        self.cursor_col = self.lines[self.cursor_line].len();
    }

    /// Get all text as a single string
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Check if input is empty
    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    /// Clear the input
    pub fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.cursor_line = 0;
        self.cursor_col = 0;
    }

    /// Get number of lines
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Set focus state
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Render the input widget
    pub fn render(&self, frame: &mut Frame, area: Rect, title: &str) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if self.focused {
                Color::Rgb(129, 199, 132) // Green when focused
            } else {
                Color::Rgb(96, 125, 139) // Gray when not
            }))
            .title(Span::styled(
                format!(" {} ", title),
                Style::default()
                    .fg(Color::Rgb(224, 247, 250))
                    .add_modifier(Modifier::BOLD),
            ));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Render text with cursor
        let mut display_lines: Vec<Line> = Vec::new();

        for (i, line) in self.lines.iter().enumerate() {
            if i == self.cursor_line && self.focused {
                // Line with cursor
                let before = &line[..self.cursor_col.min(line.len())];
                let cursor_char = line.chars().nth(self.cursor_col).unwrap_or(' ');
                let after = if self.cursor_col < line.len() {
                    &line[self.cursor_col + 1..]
                } else {
                    ""
                };

                display_lines.push(Line::from(vec![
                    Span::raw(before.to_string()),
                    Span::styled(
                        cursor_char.to_string(),
                        Style::default()
                            .bg(Color::Rgb(129, 199, 132))
                            .fg(Color::Black),
                    ),
                    Span::raw(after.to_string()),
                ]));
            } else {
                display_lines.push(Line::from(line.as_str()));
            }
        }

        let paragraph = Paragraph::new(display_lines);
        frame.render_widget(paragraph, inner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_text() {
        let mut input = SimpleInput::new();
        input.insert_char('H');
        input.insert_char('i');
        assert_eq!(input.text(), "Hi");
    }

    #[test]
    fn test_newline() {
        let mut input = SimpleInput::new();
        input.insert_char('a');
        input.insert_newline();
        input.insert_char('b');
        assert_eq!(input.text(), "a\nb");
        assert_eq!(input.line_count(), 2);
    }

    #[test]
    fn test_backspace() {
        let mut input = SimpleInput::new();
        input.insert_char('a');
        input.insert_char('b');
        input.backspace();
        assert_eq!(input.text(), "a");
    }

    #[test]
    fn test_clear() {
        let mut input = SimpleInput::new();
        input.insert_char('x');
        input.clear();
        assert!(input.is_empty());
    }
}
