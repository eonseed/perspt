//! Task Tree Component
//!
//! Displays the SRBN Task DAG as an interactive tree view with expand/collapse support.

use crate::theme::Theme;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};
use std::collections::{HashMap, HashSet};

/// Node status for display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Verifying,
    Completed,
    Failed,
    Escalated,
}

impl TaskStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "○",
            TaskStatus::Running => "◐",
            TaskStatus::Verifying => "◑",
            TaskStatus::Completed => "●",
            TaskStatus::Failed => "✗",
            TaskStatus::Escalated => "⚠",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            TaskStatus::Pending => Color::Rgb(120, 144, 156), // Gray
            TaskStatus::Running => Color::Rgb(255, 183, 77),  // Amber
            TaskStatus::Verifying => Color::Rgb(129, 212, 250), // Light blue
            TaskStatus::Completed => Color::Rgb(102, 187, 106), // Green
            TaskStatus::Failed => Color::Rgb(239, 83, 80),    // Red
            TaskStatus::Escalated => Color::Rgb(186, 104, 200), // Purple
        }
    }
}

/// A task node for the tree view
#[derive(Debug, Clone)]
pub struct TaskNode {
    /// Unique identifier
    pub id: String,
    /// Task goal/description
    pub goal: String,
    /// Current status
    pub status: TaskStatus,
    /// Depth in tree (for indentation)
    pub depth: usize,
    /// Parent node ID (None for root)
    pub parent_id: Option<String>,
    /// Whether this node has children
    pub has_children: bool,
    /// Lyapunov energy (if available)
    pub energy: Option<f32>,
}

/// Task tree viewer state with expand/collapse support
pub struct TaskTree {
    /// All task nodes indexed by ID
    nodes: HashMap<String, TaskNode>,
    /// Root node IDs (top-level tasks)
    roots: Vec<String>,
    /// Currently collapsed node IDs
    collapsed: HashSet<String>,
    /// Flattened visible list for display
    visible_tasks: Vec<String>,
    /// Selection state
    pub state: ListState,
    /// Theme for styling
    theme: Theme,
}

impl Default for TaskTree {
    fn default() -> Self {
        Self {
            nodes: HashMap::new(),
            roots: Vec::new(),
            collapsed: HashSet::new(),
            visible_tasks: Vec::new(),
            state: ListState::default(),
            theme: Theme::default(),
        }
    }
}

impl TaskTree {
    /// Create a new task tree
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a task to the tree (legacy API for compatibility)
    pub fn add_task(&mut self, id: String, goal: String, depth: usize) {
        let node = TaskNode {
            id: id.clone(),
            goal,
            status: TaskStatus::Pending,
            depth,
            parent_id: None,
            has_children: false,
            energy: None,
        };

        if depth == 0 {
            self.roots.push(id.clone());
        }

        self.nodes.insert(id, node);
        self.rebuild_visible();
    }

    /// Add a task with parent relationship
    pub fn add_task_with_parent(
        &mut self,
        id: String,
        goal: String,
        parent_id: Option<String>,
        depth: usize,
    ) {
        // Mark parent as having children
        if let Some(ref pid) = parent_id {
            if let Some(parent) = self.nodes.get_mut(pid) {
                parent.has_children = true;
            }
        }

        let is_root = parent_id.is_none();
        let node = TaskNode {
            id: id.clone(),
            goal,
            status: TaskStatus::Pending,
            depth,
            parent_id,
            has_children: false,
            energy: None,
        };

        if is_root {
            self.roots.push(id.clone());
        }

        self.nodes.insert(id, node);
        self.rebuild_visible();
    }

    /// Clear all tasks
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.roots.clear();
        self.collapsed.clear();
        self.visible_tasks.clear();
        self.state.select(None);
    }

    /// Update task status
    pub fn update_status(&mut self, id: &str, status: TaskStatus) {
        if let Some(task) = self.nodes.get_mut(id) {
            task.status = status;
        }
    }

    /// Update task energy
    pub fn update_energy(&mut self, id: &str, energy: f32) {
        if let Some(task) = self.nodes.get_mut(id) {
            task.energy = Some(energy);
        }
    }

    /// Toggle collapse state for selected node
    pub fn toggle_collapse(&mut self) {
        if let Some(selected) = self.state.selected() {
            if let Some(id) = self.visible_tasks.get(selected).cloned() {
                if let Some(node) = self.nodes.get(&id) {
                    if node.has_children {
                        if self.collapsed.contains(&id) {
                            self.collapsed.remove(&id);
                        } else {
                            self.collapsed.insert(id);
                        }
                        self.rebuild_visible();
                    }
                }
            }
        }
    }

    /// Expand all nodes
    pub fn expand_all(&mut self) {
        self.collapsed.clear();
        self.rebuild_visible();
    }

    /// Collapse all nodes
    pub fn collapse_all(&mut self) {
        for (id, node) in &self.nodes {
            if node.has_children {
                self.collapsed.insert(id.clone());
            }
        }
        self.rebuild_visible();
    }

    /// Rebuild the visible task list based on collapse state
    fn rebuild_visible(&mut self) {
        self.visible_tasks.clear();

        // Sort tasks by depth for proper tree structure
        let mut sorted: Vec<_> = self.nodes.values().collect();
        sorted.sort_by(|a, b| a.depth.cmp(&b.depth).then_with(|| a.id.cmp(&b.id)));

        // Build parent-children map
        let mut children_map: HashMap<Option<String>, Vec<String>> = HashMap::new();
        for node in sorted {
            children_map
                .entry(node.parent_id.clone())
                .or_default()
                .push(node.id.clone());
        }

        // DFS traversal respecting collapse state
        fn dfs(
            node_id: &str,
            nodes: &HashMap<String, TaskNode>,
            children_map: &HashMap<Option<String>, Vec<String>>,
            collapsed: &HashSet<String>,
            result: &mut Vec<String>,
        ) {
            result.push(node_id.to_string());

            if collapsed.contains(node_id) {
                return; // Skip children if collapsed
            }

            if let Some(children) = children_map.get(&Some(node_id.to_string())) {
                for child_id in children {
                    if nodes.contains_key(child_id) {
                        dfs(child_id, nodes, children_map, collapsed, result);
                    }
                }
            }
        }

        // Start from roots
        if let Some(root_children) = children_map.get(&None) {
            for root_id in root_children {
                dfs(
                    root_id,
                    &self.nodes,
                    &children_map,
                    &self.collapsed,
                    &mut self.visible_tasks,
                );
            }
        }
    }

    /// Select next task
    pub fn next(&mut self) {
        let len = self.visible_tasks.len();
        if len == 0 {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i >= len - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    /// Select previous task
    pub fn previous(&mut self) {
        let len = self.visible_tasks.len();
        if len == 0 {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    len - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    /// Get the currently selected task
    pub fn selected_task(&self) -> Option<&TaskNode> {
        self.state
            .selected()
            .and_then(|i| self.visible_tasks.get(i))
            .and_then(|id| self.nodes.get(id))
    }

    /// Render the task tree
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .visible_tasks
            .iter()
            .filter_map(|id| self.nodes.get(id))
            .map(|task| {
                // Build tree connector characters
                let indent = "  ".repeat(task.depth);
                let collapse_indicator = if task.has_children {
                    if self.collapsed.contains(&task.id) {
                        "▶ " // Collapsed
                    } else {
                        "▼ " // Expanded
                    }
                } else {
                    "  " // Leaf node
                };

                let icon = task.status.icon();
                let color = task.status.color();
                let goal = truncate(&task.goal, 35);

                // Build styled spans
                let mut spans = vec![
                    Span::styled(indent, Style::default().fg(Color::DarkGray)),
                    Span::styled(collapse_indicator, Style::default().fg(Color::Cyan)),
                    Span::styled(format!("{} ", icon), Style::default().fg(color)),
                ];

                // Add energy if available
                if let Some(energy) = task.energy {
                    let energy_style = self.theme.energy_style(energy);
                    spans.push(Span::styled(format!("[{:.2}] ", energy), energy_style));
                }

                spans.push(Span::styled(
                    format!("{}: ", task.id),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(goal, Style::default().fg(Color::White)));

                ListItem::new(Line::from(spans))
            })
            .collect();

        let title = format!(
            "🌳 Task DAG ({} nodes{})",
            self.visible_tasks.len(),
            if !self.collapsed.is_empty() {
                format!(", {} collapsed", self.collapsed.len())
            } else {
                String::new()
            }
        );

        let list = List::new(items)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(96, 125, 139))),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(55, 71, 79))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("→ ");

        frame.render_stateful_widget(list, area, &mut self.state);
    }
}

/// Truncate a string to max length with ellipsis
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        format!(
            "{}...",
            s.chars().take(max.saturating_sub(3)).collect::<String>()
        )
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_tasks() {
        let mut tree = TaskTree::new();
        tree.add_task("root".to_string(), "Root task".to_string(), 0);
        tree.add_task("child1".to_string(), "Child 1".to_string(), 1);

        assert_eq!(tree.nodes.len(), 2);
        assert_eq!(tree.visible_tasks.len(), 2);
    }

    #[test]
    fn test_update_status() {
        let mut tree = TaskTree::new();
        tree.add_task("task1".to_string(), "Test".to_string(), 0);
        tree.update_status("task1", TaskStatus::Running);

        assert_eq!(tree.nodes.get("task1").unwrap().status, TaskStatus::Running);
    }

    #[test]
    fn test_navigation() {
        let mut tree = TaskTree::new();
        tree.add_task("t1".to_string(), "Task 1".to_string(), 0);
        tree.add_task("t2".to_string(), "Task 2".to_string(), 0);
        tree.add_task("t3".to_string(), "Task 3".to_string(), 0);

        assert!(tree.state.selected().is_none());

        tree.next();
        assert_eq!(tree.state.selected(), Some(0));

        tree.next();
        assert_eq!(tree.state.selected(), Some(1));

        tree.previous();
        assert_eq!(tree.state.selected(), Some(0));
    }
}
