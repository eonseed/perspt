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
            let char_idx = self.cursor_col.min(line.chars().count());
            let byte_idx = line
                .char_indices()
                .map(|(b, _)| b)
                .nth(char_idx)
                .unwrap_or(line.len());
            line.insert(byte_idx, c);
            self.cursor_col += 1;
        }
    }

    /// Insert a newline at the cursor position
    pub fn insert_newline(&mut self) {
        if self.cursor_line < self.lines.len() {
            let line = &mut self.lines[self.cursor_line];
            let char_idx = self.cursor_col.min(line.chars().count());
            let byte_idx = line
                .char_indices()
                .map(|(b, _)| b)
                .nth(char_idx)
                .unwrap_or(line.len());

            // Split the current line at cursor
            let remainder = line[byte_idx..].to_string();
            line.truncate(byte_idx);

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
            let char_idx = (self.cursor_col - 1).min(line.chars().count() - 1);
            let byte_idx = line
                .char_indices()
                .map(|(b, _)| b)
                .nth(char_idx)
                .unwrap_or(0);
            line.remove(byte_idx);
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            // Merge with previous line
            let current_line = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            self.cursor_col = self.lines[self.cursor_line].chars().count();
            self.lines[self.cursor_line].push_str(&current_line);
        }
    }

    /// Delete character at cursor (delete key)
    pub fn delete(&mut self) {
        let line = &mut self.lines[self.cursor_line];
        let char_count = line.chars().count();
        if self.cursor_col < char_count {
            let byte_idx = line
                .char_indices()
                .map(|(b, _)| b)
                .nth(self.cursor_col)
                .unwrap_or(0);
            line.remove(byte_idx);
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
            self.cursor_col = self.lines[self.cursor_line].chars().count();
        }
    }

    /// Move cursor right
    pub fn move_right(&mut self) {
        let char_count = self.lines[self.cursor_line].chars().count();
        if self.cursor_col < char_count {
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
            self.cursor_col = self
                .cursor_col
                .min(self.lines[self.cursor_line].chars().count());
        }
    }

    /// Move cursor down
    pub fn move_down(&mut self) {
        if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.cursor_col = self
                .cursor_col
                .min(self.lines[self.cursor_line].chars().count());
        }
    }

    /// Move cursor to start of line
    pub fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    /// Move cursor to end of line
    pub fn move_end(&mut self) {
        self.cursor_col = self.lines[self.cursor_line].chars().count();
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

    /// Calculate the number of visual lines when wrapped to the given width
    pub fn line_count_wrapped(&self, width: usize) -> usize {
        if width == 0 {
            return self.lines.len();
        }

        let mut count = 0;
        for line in &self.lines {
            if line.is_empty() {
                count += 1;
            } else {
                let wrapped = textwrap::wrap(line, width);
                count += wrapped.len().max(1);
            }
        }
        count
    }

    /// Get the current cursor line index (0-indexed)
    pub fn cursor_line(&self) -> usize {
        self.cursor_line
    }

    /// Set all text in the input
    pub fn set_text(&mut self, text: &str) {
        self.lines = text.lines().map(|s| s.to_string()).collect();
        if self.lines.is_empty() {
            self.lines = vec![String::new()];
        }
        self.cursor_line = self.lines.len() - 1;
        self.cursor_col = self.lines[self.cursor_line].chars().count();
    }

    /// Kill (delete) from current cursor to end of the line
    pub fn kill_to_end(&mut self) {
        if self.cursor_line < self.lines.len() {
            let line = &mut self.lines[self.cursor_line];
            let char_count = line.chars().count();
            let char_idx = self.cursor_col.min(char_count);
            let byte_idx = line
                .char_indices()
                .map(|(b, _)| b)
                .nth(char_idx)
                .unwrap_or(line.len());
            line.truncate(byte_idx);
            self.cursor_col = line.chars().count();
        }
    }

    /// Kill (delete) from current cursor to start of the line
    pub fn kill_to_start(&mut self) {
        if self.cursor_line < self.lines.len() {
            let line = &mut self.lines[self.cursor_line];
            let char_count = line.chars().count();
            let char_idx = self.cursor_col.min(char_count);
            let byte_idx = line
                .char_indices()
                .map(|(b, _)| b)
                .nth(char_idx)
                .unwrap_or(line.len());
            let remainder = line[byte_idx..].to_string();
            *line = remainder;
            self.cursor_col = 0;
        }
    }

    /// Delete the word or whitespace segment immediately before the cursor
    pub fn delete_word_before(&mut self) {
        if self.cursor_line < self.lines.len() {
            let line = &mut self.lines[self.cursor_line];
            let char_count = line.chars().count();
            let cursor_idx = self.cursor_col.min(char_count);
            if cursor_idx == 0 {
                if self.cursor_line > 0 {
                    let current_line = self.lines.remove(self.cursor_line);
                    self.cursor_line -= 1;
                    self.cursor_col = self.lines[self.cursor_line].chars().count();
                    self.lines[self.cursor_line].push_str(&current_line);
                }
                return;
            }

            let chars: Vec<char> = line.chars().take(cursor_idx).collect();
            let mut i = chars.len();

            // Skip trailing whitespace/punctuation first
            while i > 0 && (chars[i - 1].is_whitespace() || !chars[i - 1].is_alphanumeric()) {
                i -= 1;
            }
            // Skip the word characters
            while i > 0 && chars[i - 1].is_alphanumeric() {
                i -= 1;
            }

            let before: String = chars[..i].iter().collect();
            let after: String = line.chars().skip(cursor_idx).collect();
            *line = format!("{}{}", before, after);
            self.cursor_col = i;
        }
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
                // Line with cursor using character-based slicing
                let char_count = line.chars().count();
                let cursor_idx = self.cursor_col.min(char_count);

                let before: String = line.chars().take(cursor_idx).collect();
                let cursor_char = line.chars().nth(cursor_idx).unwrap_or(' ');
                let after: String = line.chars().skip(cursor_idx + 1).collect();

                display_lines.push(Line::from(vec![
                    Span::raw(before),
                    Span::styled(
                        cursor_char.to_string(),
                        Style::default()
                            .bg(Color::Rgb(129, 199, 132))
                            .fg(Color::Black),
                    ),
                    Span::raw(after),
                ]));
            } else {
                display_lines.push(Line::from(line.as_str()));
            }
        }

        let paragraph = Paragraph::new(display_lines).wrap(ratatui::widgets::Wrap { trim: false });
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

    #[test]
    fn test_set_text() {
        let mut input = SimpleInput::new();
        input.set_text("hello\nworld");
        assert_eq!(input.text(), "hello\nworld");
        assert_eq!(input.cursor_line(), 1);
        assert_eq!(input.cursor_col, 5);
    }

    #[test]
    fn test_unicode_editing() {
        let mut input = SimpleInput::new();
        input.insert_char('न');
        input.insert_char('म');
        input.insert_char('स');
        input.insert_char('्');
        input.insert_char('त');
        input.insert_char('े');
        assert_eq!(input.text(), "नमस्ते");
        assert_eq!(input.cursor_col, 6);

        // move left 3 times
        input.move_left();
        input.move_left();
        input.move_left();
        assert_eq!(input.cursor_col, 3);

        // delete one char
        input.delete();
        assert_eq!(input.text(), "नमसत\u{947}");
    }

    #[test]
    fn test_emacs_actions() {
        let mut input = SimpleInput::new();
        input.set_text("hello big world");
        input.cursor_col = 9; // space after "big"
        input.delete_word_before();
        assert_eq!(input.text(), "hello  world");
        assert_eq!(input.cursor_col, 6);

        input.kill_to_end();
        assert_eq!(input.text(), "hello ");

        input.set_text("hello world");
        input.cursor_col = 5;
        input.kill_to_start();
        assert_eq!(input.text(), " world");
    }

    #[test]
    fn test_line_count_wrapped() {
        let mut input = SimpleInput::new();
        input.set_text("This is a very long string that should wrap when given a small width.");
        let wrapped_lines = input.line_count_wrapped(10);
        assert!(wrapped_lines > 1);
    }
}
