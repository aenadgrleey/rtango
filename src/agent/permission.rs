/// Canonical tool permission that can be mapped between agent schemas.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Read files (Claude Code: `Read`, Copilot: `read`)
    Read,
    /// Write/create files (Claude Code: `Write`, Copilot: `write`)
    Write,
    /// Edit existing files (Claude Code: `Edit`, Copilot: `edit`)
    Edit,
    /// Execute shell commands with optional scope pattern
    /// (Claude Code: `Bash(pattern)`, Copilot: `shell`)
    Shell(Option<String>),
    /// Search file contents (Claude Code: `Grep`, Copilot: `grep`)
    Grep,
    /// Find files by pattern (Claude Code: `Glob`, Copilot: `glob`)
    Glob,
    /// Fetch web content (Claude Code: `WebFetch`, Copilot: `web_fetch`)
    WebFetch,
    /// Search the web (Claude Code: `WebSearch`, Copilot: `web_search`)
    WebSearch,
    /// Edit notebooks (Claude Code only)
    NotebookEdit,
    /// Write/manage todos (Claude Code only)
    TodoWrite,
    /// Tool that doesn't map to a known canonical permission
    Other(String),
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read => write!(f, "Read"),
            Self::Write => write!(f, "Write"),
            Self::Edit => write!(f, "Edit"),
            Self::Shell(None) => write!(f, "Shell"),
            Self::Shell(Some(p)) => write!(f, "Shell({p})"),
            Self::Grep => write!(f, "Grep"),
            Self::Glob => write!(f, "Glob"),
            Self::WebFetch => write!(f, "WebFetch"),
            Self::WebSearch => write!(f, "WebSearch"),
            Self::NotebookEdit => write!(f, "NotebookEdit"),
            Self::TodoWrite => write!(f, "TodoWrite"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}
