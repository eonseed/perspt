PSP: 000002
Title: Multi-line Input Support with Enhanced Navigation
Author: Vikrant Rathore
Status: Draft
Type: UI/UX
Created: 2025-01-14
Discussion-To: Later

========
Abstract
========

This PSP proposes enhancing the input area to support multi-line text input using Shift+Enter for line breaks, while preserving Enter for message sending. Users will be able to navigate within multi-line input using arrow keys and compose complex messages directly in the interface without scrollable input area.

==========
Motivation
==========

**What user need or pain point does this address?**

Currently, users cannot compose multi-line messages, paste code blocks, or format complex queries directly in Perspt's input area. When Enter is pressed, the message is immediately sent, making it impossible to:

* Write multi-paragraph questions or explanations
* Paste and review multi-line code snippets before sending
* Compose formatted messages with proper line breaks
* Edit longer messages within the input area

This affects all users who need to send anything beyond single-line messages, forcing them to use external editors or send multiple fragmented messages.

================
Proposed Changes
================

Functional Specification
========================

**Behavioral Changes:**

* **Shift+Enter**: Creates a new line in the input area, expanding it vertically
* **Enter**: Sends the complete multi-line message (unchanged behavior)
* **Arrow Keys**: Navigate within multi-line input (Up/Down between lines, Left/Right within lines)
* **Input Area**: Dynamically expands from 3 to 8 lines maximum based on content
* **Cursor Navigation**: 2D cursor movement with proper line boundaries
* **Save Command**: `/save` command preserves multi-line formatting in exported files

UI/UX Design
============

**User Goals:** Enable composing and editing complex multi-line messages directly in the chat interface.

**Interaction Flow:**

1. User types normally, input behaves as single-line
2. User presses Shift+Enter to create new lines
3. Input area expands vertically (max 5 content lines)
4. Arrow keys allow navigation within the multi-line text
5. Enter sends the complete message preserving line breaks

**Visual Design:**

* Input area height dynamically adjusts from 3 to 8 lines maximum
* Visible text window shows content around cursor position
* Cursor position clearly visible across multiple lines
* Line count indicator when in multi-line mode

**Accessibility Considerations:**

* Keyboard navigation remains intuitive and standard
* Screen readers can announce line breaks and cursor position
* No color-only indicators for multi-line state
* Fallback to single-line behavior on limited terminals

Technical Specification
=======================


**Key Implementation Changes:**
-------------------------------


**In `ui.rs` - App struct modifications:**

.. code-block:: rust

   // filepath: /Users/vikrantrathore/projects/eonseed/perspt/src/ui.rs
   pub struct App {
       // ...existing fields...
       
       // Replace single cursor with 2D coordinates
       pub input_cursor_line: usize,
       pub input_cursor_column: usize,
       
       // Multi-line input support
       pub input_lines: Vec<String>,
       pub input_view_start_line: usize,
       
       // Remove old single-line fields
       // pub input_text: String,  // Remove this
       // pub cursor_position: usize,  // Remove this
       // pub input_scroll_offset: usize,  // Remove this
       
       // ...rest of existing fields...
   }

   impl App {
       pub fn new(config: AppConfig) -> Self {
           Self {
               // ...existing initialization...
               
               // Initialize multi-line input
               input_cursor_line: 0,
               input_cursor_column: 0,
               input_lines: vec![String::new()],
               input_view_start_line: 0,
               
               // ...rest of existing initialization...
           }
       }
       
       /// Check if currently in multi-line input mode
       pub fn is_in_multiline_input(&self) -> bool {
           self.input_lines.len() > 1 || self.input_lines[0].contains('\n')
       }
       
       /// Insert newline (Shift+Enter)
       pub fn insert_newline(&mut self) {
           if !self.is_input_disabled {
               let current_line = self.current_input_line().to_string();
               let (before_cursor, after_cursor) = current_line.split_at(self.input_cursor_column);
               
               // Update current line to contain only text before cursor
               self.input_lines[self.input_cursor_line] = before_cursor.to_string();
               
               // Insert new line with text after cursor
               self.input_cursor_line += 1;
               self.input_lines.insert(self.input_cursor_line, after_cursor.to_string());
               self.input_cursor_column = 0;
               
               self.update_input_view();
               self.needs_redraw = true;
           }
       }
       
       /// Move cursor between lines and within lines
       pub fn move_cursor_up(&mut self) {
           if self.input_cursor_line > 0 {
               self.input_cursor_line -= 1;
               let new_line_len = self.current_input_line().len();
               self.input_cursor_column = self.input_cursor_column.min(new_line_len);
               self.update_input_view();
               self.needs_redraw = true;
           }
       }
       
       pub fn move_cursor_down(&mut self) {
           if self.input_cursor_line + 1 < self.input_lines.len() {
               self.input_cursor_line += 1;
               let new_line_len = self.current_input_line().len();
               self.input_cursor_column = self.input_cursor_column.min(new_line_len);
               self.update_input_view();
               self.needs_redraw = true;
           }
       }
       
       /// Update input view to keep cursor visible
       fn update_input_view(&mut self) {
           let max_visible_lines = 5; // Show max 5 lines of input
           
           // Ensure cursor line is visible in the view
           if self.input_cursor_line < self.input_view_start_line {
               self.input_view_start_line = self.input_cursor_line;
           } else if self.input_cursor_line >= self.input_view_start_line + max_visible_lines {
               self.input_view_start_line = self.input_cursor_line.saturating_sub(max_visible_lines - 1);
           }
       }
       
       /// Get visible input lines and cursor position for rendering
       pub fn get_visible_input(&self) -> (Vec<&str>, usize, usize) {
           let max_visible_lines = 5;
           let end_line = (self.input_view_start_line + max_visible_lines).min(self.input_lines.len());
           
           let visible_lines: Vec<&str> = self.input_lines[self.input_view_start_line..end_line]
               .iter()
               .map(|s| s.as_str())
               .collect();
           
           let cursor_line_in_view = self.input_cursor_line.saturating_sub(self.input_view_start_line);
           
           (visible_lines, cursor_line_in_view, self.input_cursor_column)
       }
       
       /// Get complete input text for sending (joins all lines)
       pub fn take_input(&mut self) -> Option<String> {
           let input_text = self.input_lines.join("\n").trim().to_string();
           if input_text.is_empty() {
               None
           } else {
               self.clear_input();
               Some(input_text)
           }
       }
       
       /// Get current input height for dynamic layout
       pub fn get_input_height(&self) -> u16 {
           let content_lines = self.input_lines.len().min(5); // Max 5 visible lines
           (content_lines + 2).max(3) as u16 // +2 for borders, minimum 3
       }
   }

**Key handling modifications in `handle_terminal_event()`:**

.. code-block:: rust

   // filepath: /Users/vikrantrathore/projects/eonseed/perspt/src/ui.rs
   async fn handle_terminal_event(
       provider: &Arc<GenAIProvider>,
   ) -> Option<AppEvent> {
       if let Ok(Event::Key(key)) = event::read() {
           match key.code {
               KeyCode::Enter => {
                   if key.modifiers.contains(KeyModifiers::SHIFT) {
                       // Shift+Enter: Insert newline
                       return Some(AppEvent::InsertNewline);
                   } else {
                       // Regular Enter: Send message
                       return Some(AppEvent::SendMessage);
                   }
               }
               KeyCode::Up => {
                   if app.is_in_multiline_input() && !app.is_input_disabled {
                       return Some(AppEvent::MoveCursorUp);
                   } else {
                       return Some(AppEvent::ScrollUp);
                   }
               }
               KeyCode::Down => {
                   if app.is_in_multiline_input() && !app.is_input_disabled {
                       return Some(AppEvent::MoveCursorDown);
                   } else {
                       return Some(AppEvent::ScrollDown);
                   }
               }
               KeyCode::Left => {
                   if !app.is_input_disabled {
                       return Some(AppEvent::MoveCursorLeft);
                   }
               }
               KeyCode::Right => {
                   if !app.is_input_disabled {
                       return Some(AppEvent::MoveCursorRight);
                   }
               }
               KeyCode::Home => {
                   if !app.is_input_disabled && key.modifiers.contains(KeyModifiers::CONTROL) {
                       return Some(AppEvent::MoveCursorToStart);
                   } else if !app.is_input_disabled {
                       return Some(AppEvent::MoveCursorToLineStart);
                   } else {
                       return Some(AppEvent::ScrollToTop);
                   }
               }
               KeyCode::End => {
                   if !app.is_input_disabled && key.modifiers.contains(KeyModifiers::CONTROL) {
                       return Some(AppEvent::MoveCursorToEnd);
                   } else if !app.is_input_disabled {
                       return Some(AppEvent::MoveCursorToLineEnd);
                   } else {
                       return Some(AppEvent::ScrollToBottom);
                   }
               }
               // ...existing key handling...
           }
       }
       None
   }
           }
       }
       None
   }

**Update AppEvent enum:**

.. code-block:: rust

   // filepath: /Users/vikrantrathore/projects/eonseed/perspt/src/ui.rs
   #[derive(Debug)]
   pub enum AppEvent {
       // ...existing events...
       
       // New multi-line input events
       InsertNewline,
       MoveCursorUp,
       MoveCursorDown,
       MoveCursorLeft,
       MoveCursorRight,
       MoveCursorToLineStart,
       MoveCursorToLineEnd,
       MoveCursorToStart,
       MoveCursorToEnd,
   }

**Enhanced input area rendering:**

.. code-block:: rust

   // filepath: /Users/vikrantrathore/projects/eonseed/perspt/src/ui.rs
   fn draw_enhanced_input_area(f: &mut Frame, area: Rect, app: &App) {
       let input_height = app.get_input_height();
       
       let input_chunks = Layout::default()
           .direction(Direction::Vertical)
           .constraints([
               Constraint::Length(input_height),  // Dynamic height
               Constraint::Length(2),  // Progress bar or hint
           ])
           .split(area);

       let (visible_lines, cursor_line, cursor_column) = app.get_visible_input();
       
       // Input field styling based on state
       let (border_color, title) = if app.is_input_disabled {
           (Color::DarkGray, " Input (Disabled - AI is thinking...) ")
       } else if visible_lines.len() > 1 {
           (Color::Green, format!(" Multi-line Input ({} lines) - Enter to send, F1 for help ", visible_lines.len()))
       } else {
           (Color::Green, " Type your message (Shift+Enter for new line, Enter to send) ")
       };

       // Create multi-line input content with cursor
       let mut input_content: Vec<Line> = Vec::new();
       
       for (line_idx, line_text) in visible_lines.iter().enumerate() {
           let mut line_spans = Vec::new();
           
           if line_idx == cursor_line {
               // This is the line with the cursor
               let before_cursor = &line_text[..cursor_column.min(line_text.len())];
               let at_cursor = line_text.chars().nth(cursor_column).unwrap_or(' ');
               let after_cursor = &line_text[cursor_column.min(line_text.len())..];

               if !before_cursor.is_empty() {
                   line_spans.push(Span::styled(before_cursor, Style::default().fg(Color::White)));
               }

               // Cursor character with highlighting and blinking
               if !app.is_input_disabled {
                   let cursor_style = if app.cursor_blink_state {
                       Style::default().fg(Color::Black).bg(Color::White)
                   } else {
                       Style::default().fg(Color::White).bg(Color::DarkGray)
                   };
                   
                   line_spans.push(Span::styled(at_cursor.to_string(), cursor_style));
               }

               if !after_cursor.is_empty() {
                   line_spans.push(Span::styled(after_cursor, Style::default().fg(Color::White)));
               }
           } else {
               // Regular line without cursor
               line_spans.push(Span::styled(*line_text, Style::default().fg(Color::White)));
           }
           
           input_content.push(Line::from(line_spans));
       }

       let input_paragraph = Paragraph::new(input_content)
           .block(Block::default()
               .borders(Borders::ALL)
               .border_type(BorderType::Rounded)
               .border_style(Style::default().fg(border_color))
               .title(title)
               .title_style(Style::default().fg(border_color)));

       f.render_widget(input_paragraph, input_chunks[0]);
   }

**Update main UI layout to use dynamic input height:**

.. code-block:: rust

   // filepath: /Users/vikrantrathore/projects/eonseed/perspt/src/ui.rs
   fn draw_enhanced_ui(f: &mut Frame, app: &mut App, model_name: &str) {
       // Update terminal dimensions
       app.terminal_height = f.area().height as usize;
       app.terminal_width = f.area().width as usize;
       
       let input_height = app.get_input_height() + 2; // +2 for progress bar area
       
       let main_chunks = Layout::default()
           .direction(Direction::Vertical)
           .constraints([
               Constraint::Length(3),  // Header
               Constraint::Min(1),     // Chat area (flexible)
               Constraint::Length(input_height),  // Dynamic input area
               Constraint::Length(3),  // Status line
           ])
           .split(f.area());

       // Update input width for proper calculations
       app.input_width = main_chunks[2].width as usize;

       // Render components
       draw_enhanced_header(f, main_chunks[0], model_name, app);
       draw_enhanced_chat_area(f, main_chunks[1], app);
       draw_enhanced_input_area(f, main_chunks[2], app);
       draw_enhanced_status_line(f, main_chunks[3], app);
   }

**Event handling in main UI loop:**

.. code-block:: rust

   // filepath: /Users/vikrantrathore/projects/eonseed/perspt/src/ui.rs
   // In the main event handling match statement
   match event {
       AppEvent::InsertNewline => {
           app.insert_newline();
       }
       AppEvent::MoveCursorUp => {
           app.move_cursor_up();
       }
       AppEvent::MoveCursorDown => {
           app.move_cursor_down();
       }
       AppEvent::MoveCursorLeft => {
           app.move_cursor_left();
       }
       AppEvent::MoveCursorRight => {
           app.move_cursor_right();
       }
       AppEvent::MoveCursorToLineStart => {
           app.move_cursor_to_line_start();
       }
       AppEvent::MoveCursorToLineEnd => {
           app.move_cursor_to_line_end();
       }
       // ...existing event handling...
   }

**Save command enhancement (already compatible):**

The existing `save_conversation()` method in `App` already uses `raw_content` field which will preserve the multi-line formatting automatically when messages are sent with newline characters.

==========================
Documentation Requirements
==========================

This PSP will require updates to several documentation files:

.. rubric:: **User Guide (`docs/user-guide.md`)**

**Section: "Using the Chat Interface"**

Add new subsection: "Multi-line Input Support"

.. code-block:: markdown

   ### Multi-line Input Support

   Perspt supports multi-line input for composing complex messages, code blocks, and formatted text:

   **Creating Multi-line Messages:**
   - Press `Shift+Enter` to create a new line in your message
   - Press `Enter` to send the complete multi-line message
   - The input area expands automatically (up to 5 visible lines)

   **Navigation in Multi-line Input:**
   - `↑/↓ Arrow Keys`: Move cursor between lines
   - `←/→ Arrow Keys`: Move cursor within the current line
   - `Home`: Move to beginning of current line
   - `End`: Move to end of current line
   - `Ctrl+Home`: Move to start of entire input
   - `Ctrl+End`: Move to end of entire input

   **Visual Indicators:**
   - Input area shows line count when multi-line: "Multi-line Input (3 lines)"
   - Cursor position is clearly visible across lines
   - Input hint shows "Shift+Enter for new line, Enter to send"

   **Use Cases:**
   - Composing detailed questions with multiple paragraphs
   - Pasting and editing code blocks before sending
   - Writing formatted explanations with line breaks
   - Reviewing longer messages before submission

**Section: "Keyboard Shortcuts"**

Update the keyboard shortcuts table:

.. code-block:: markdown

   | Key Combination | Action | Context |
   |-----------------|--------|---------|
   | Enter | Send message | Input area |
   | Shift+Enter | Insert new line | Input area |
   | ↑/↓ | Navigate lines OR scroll chat | Input area/Chat area |
   | ←/→ | Move cursor within line | Input area |
   | Home | Beginning of current line | Input area |
   | End | End of current line | Input area |
   | Ctrl+Home | Start of entire input | Input area |
   | Ctrl+End | End of entire input | Input area |

.. rubric:: **Developer Guide (`docs/developer-guide.md`)**

**Section: "UI Architecture"**

Add subsection: "Multi-line Input Implementation"

.. code-block:: markdown

   ### Multi-line Input Implementation

   The multi-line input system uses a 2D coordinate system for cursor management:

   **Key Components:**
   - `input_lines: Vec<String>` - Stores each line of input separately
   - `input_cursor_line: usize` - Current line number (0-based)
   - `input_cursor_column: usize` - Current column position within line
   - `input_view_start_line: usize` - First visible line in input area

   **Core Methods:**
   - `insert_newline()` - Splits current line at cursor position
   - `move_cursor_up()/move_cursor_down()` - Navigate between lines
   - `update_input_view()` - Ensures cursor remains visible
   - `get_visible_input()` - Returns lines for rendering

   **Event Flow:**
   1. `Shift+Enter` → `AppEvent::InsertNewline` → `insert_newline()`
   2. Arrow keys → Movement events → Cursor position updates
   3. Regular `Enter` → Message sending with `join("\n")`

   **Rendering Logic:**
   - Dynamic height calculation based on content lines
   - 2D cursor rendering with proper highlighting
   - Line-aware text wrapping and display

**Section: "Testing Guidelines"**

Add testing scenarios:

.. code-block:: markdown

   ### Multi-line Input Testing

   **Test Scenarios:**
   - Insert newlines with Shift+Enter
   - Navigate between lines using arrow keys
   - Cursor position at line boundaries
   - Backspace/Delete across line breaks
   - Copy/paste multi-line content
   - Save functionality with line breaks
   - Dynamic input area height changes
   - Terminal resize with multi-line input

.. rubric:: **Quick Start Guide (`docs/quickstart.md`)**

**Section: "Basic Usage"**

Add note about multi-line support:

.. code-block:: markdown

   ### Composing Messages

   - Type your message in the input area at the bottom
   - Press `Enter` to send your message
   - **For multi-line messages**: Press `Shift+Enter` to create new lines, then `Enter` to send
   - Use arrow keys to navigate and edit longer messages

   **Tip:** For code blocks or detailed explanations, use `Shift+Enter` to format your message across multiple lines before sending.

.. rubric:: **Help System (`src/ui.rs` - help overlay)**

Update the built-in help overlay:

.. code-block:: rust

   // In draw_help_overlay() function
   let help_text = vec![
       "━━━━━━━━━━━━━━━━━━━━ Perspt Help ━━━━━━━━━━━━━━━━━━━━",
       "",
       "💬 MESSAGE COMPOSITION:",
       "  Enter              Send message",
       "  Shift+Enter        Create new line",
       "  ↑/↓ Arrows         Navigate lines (in multi-line input)",
       "  ←/→ Arrows         Move cursor within line", 
       "  Home/End           Beginning/end of current line",
       "  Ctrl+Home/End      Start/end of entire input",
       "",
       "📜 CHAT NAVIGATION:",
       "  ↑/↓ Arrows         Scroll chat history (when not in multi-line input)",
       "  Page Up/Down       Scroll faster",
       "  Ctrl+Home/End      Jump to top/bottom of chat",
       "",
       "💾 COMMANDS:",
       "  /save              Save conversation to file",
       "  /clear             Clear conversation history",
       "",
       "⌨️  GENERAL:",
       "  F1 or ?            Show/hide this help",
       "  Ctrl+C or Q        Quit application",
       "",
       "Press any key to close help...",
   ];

.. rubric:: **API Documentation (`src/ui.rs` module docs)**

Update the module-level documentation:

.. code-block:: rust

   //! ## Multi-line Input Features
   //!
   //! The UI module supports sophisticated multi-line input editing:
   //! * **2D Cursor Management**: Tracks both line and column positions
   //! * **Dynamic Height**: Input area expands from 3 to 8 lines based on content
   //! * **View Window**: Shows relevant content around cursor without scrolling
   //! * **Line Navigation**: Full arrow key navigation within multi-line text
   //! * **Format Preservation**: Line breaks maintained in sent messages and saved files
   //!
   //! ## Input Handling Architecture
   //!
   //! // Multi-line input uses vector of strings instead of single string
   //! pub struct App {
   //!     pub input_lines: Vec<String>,           // Each line stored separately
   //!     pub input_cursor_line: usize,           // Current line (0-based)
   //!     pub input_cursor_column: usize,         // Position within line
   //!     pub input_view_start_line: usize,       // First visible line
   //! }

.. rubric:: **README.md**

Update the features section:

.. code-block:: markdown

   ## Features

   - **Unified API**: Single interface for multiple LLM providers
   - **Real-time streaming**: Live response streaming for better user experience
   - **Multi-line Input**: Compose complex messages with Shift+Enter line breaks
   - **Advanced Navigation**: Full cursor control within multi-line text
   - **Robust error handling**: Comprehensive panic recovery and error categorization

.. rubric:: **Configuration Documentation**

No configuration changes required - this is a pure UI enhancement.

.. rubric:: **Changelog (`CHANGELOG.md`)**

Add entry for the new version:

.. code-block:: markdown

   ## [Unreleased]

   ### Added
   - Multi-line input support with Shift+Enter for line breaks
   - 2D cursor navigation within input area using arrow keys
   - Dynamic input area height (3-8 lines) based on content
   - Line count indicator for multi-line input mode
   - Enhanced keyboard shortcuts for line-based navigation

   ### Changed
   - Input area now expands vertically for multi-line content
   - Arrow key behavior is context-aware (input navigation vs chat scrolling)
   - Home/End keys work within current line, Ctrl+Home/End for entire input

   ### Technical
   - Replaced single-string input with vector-based line storage
   - Implemented 2D cursor coordinate system
   - Added view window management for input display

=========
Rationale
=========

**Design Decision Rationale:**

* **Non-scrollable input**: Maintains focus on conversation history scrolling while providing adequate editing space
* **Fixed maximum height**: Prevents input area from overwhelming the chat interface
* **2D cursor navigation**: Provides intuitive editing experience for complex messages
* **View window approach**: Shows relevant content around cursor without scrollbars

**Alternatives Considered:**

* **Scrollable input area**: Rejected - conflicts with main conversation scrolling and adds UI complexity
* **Modal editor**: Rejected - disrupts conversational flow
* **Unlimited input expansion**: Rejected - could dominate the interface

=======================
Backwards Compatibility
=======================

**User Impact:**

* **No breaking changes**: Existing single-line workflows remain identical
* **Progressive enhancement**: Multi-line capability discovered naturally through Shift+Enter
* **Preserved shortcuts**: All existing keyboard shortcuts continue to work

**Configuration Impact:**

* No configuration file changes required
* No migration needed for existing users

=========
Copyright
=========

This document is placed in the public domain or under the CC0-1.0-Universal license, whichever is more permissive.
