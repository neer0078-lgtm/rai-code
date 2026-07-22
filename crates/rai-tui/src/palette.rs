//! The command palette — fuzzy-searchable command list (T53).
//!
//! Ctrl+P / Space opens it; typing filters; Enter executes. The filtering is
//! a simple case-insensitive substring match (fuzzy scoring is a later enhancement).

/// A command in the palette.
#[derive(Debug, Clone)]
pub struct Command {
    /// The command name (shown + matched).
    pub name: String,
    /// The description.
    pub description: String,
}

/// The command palette.
#[derive(Debug, Default)]
pub struct CommandPalette {
    /// All available commands.
    pub commands: Vec<Command>,
    /// The current query.
    pub query: String,
    /// The selected index.
    pub selected: usize,
}

impl CommandPalette {
    /// Construct a palette with the default RAI Code commands.
    pub fn new() -> Self {
        Self {
            commands: vec![
                Command {
                    name: "switch mode plan".into(),
                    description: "read-only mode".into(),
                },
                Command {
                    name: "switch mode approval".into(),
                    description: "per-action approval".into(),
                },
                Command {
                    name: "switch mode bypass".into(),
                    description: "YOLO (unattended)".into(),
                },
                Command {
                    name: "open diff".into(),
                    description: "review changes".into(),
                },
                Command {
                    name: "open browser".into(),
                    description: "test/debug the app".into(),
                },
                Command {
                    name: "open plan".into(),
                    description: "task graph".into(),
                },
                Command {
                    name: "open profile".into(),
                    description: "your Hindsight profile".into(),
                },
                Command {
                    name: "open directives".into(),
                    description: "governance rules".into(),
                },
                Command {
                    name: "run task".into(),
                    description: "start a new task".into(),
                },
                Command {
                    name: "stop".into(),
                    description: "stop the current task".into(),
                },
                Command {
                    name: "help".into(),
                    description: "show the cheatsheet".into(),
                },
                Command {
                    name: "quit".into(),
                    description: "exit RAI Code".into(),
                },
            ],
            query: String::new(),
            selected: 0,
        }
    }

    /// T53: filter commands by the query (case-insensitive substring).
    pub fn filter(&self, query: &str) -> Vec<&Command> {
        let q = query.trim().to_lowercase();
        self.commands
            .iter()
            .filter(|c| q.is_empty() || c.name.to_lowercase().contains(&q))
            .collect()
    }

    /// Set the query.
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.selected = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T53: empty query returns all commands.
    #[test]
    fn palette_filter_empty_returns_all() {
        let pal = CommandPalette::new();
        let all = pal.filter("");
        assert!(!all.is_empty());
        assert!(all.iter().any(|c| c.name.contains("mode")));
    }

    /// T53: "mode" matches the mode-switching commands.
    #[test]
    fn palette_filter_mode() {
        let pal = CommandPalette::new();
        let matched = pal.filter("mode");
        assert!(matched.iter().all(|c| c.name.contains("mode")));
        assert!(matched.len() >= 3); // plan, approval, bypass
    }

    /// T53: "diff" matches "open diff" but not "open browser".
    #[test]
    fn palette_filter_diff() {
        let pal = CommandPalette::new();
        let matched = pal.filter("diff");
        assert!(matched.iter().any(|c| c.name.contains("diff")));
        assert!(!matched.iter().any(|c| c.name.contains("browser")));
    }

    /// T53: case-insensitive.
    #[test]
    fn palette_filter_case_insensitive() {
        let pal = CommandPalette::new();
        let matched = pal.filter("HELP");
        assert!(matched.iter().any(|c| c.name == "help"));
    }
}
