pub mod add;
pub mod init;
pub mod own;
pub mod status;
pub mod sync;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rtango", version, about = "Package manager for agent skills and configuration files")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Scan project and create .rtango/spec.yaml + .rtango/lock.yaml
    Init {
        /// Overwrite existing spec
        #[arg(short, long)]
        force: bool,

        /// Explicitly specify agents (repeatable)
        #[arg(short, long, value_name = "NAME")]
        agent: Vec<String>,

        /// Skip auto-detection, create minimal empty spec
        #[arg(short, long)]
        no_detect: bool,
    },

    /// Bring target files in sync with the spec
    Sync {
        /// Dry-run: exit 1 if out of sync (for CI)
        #[arg(short, long)]
        check: bool,

        /// Ignore on_target_modified: fail
        #[arg(short, long)]
        force: bool,

        /// Only process a single rule
        #[arg(short, long, value_name = "ID")]
        rule: Option<String>,

        /// Adopt existing target files on first sync
        #[arg(short, long)]
        adopt: bool,
    },

    /// Show sync plan without writing anything
    Status {
        /// Only show a single rule
        #[arg(short, long, value_name = "ID")]
        rule: Option<String>,

        /// Show up-to-date items too
        #[arg(short, long)]
        verbose: bool,
    },

    /// Record or clear a manual ownership decision for a contested path
    Own {
        /// Target path (absolute, or relative to the project root)
        path: std::path::PathBuf,

        /// Rule id that should own the path (omit with --clear)
        rule_id: Option<String>,

        /// Remove any recorded ownership for this path
        #[arg(short, long)]
        clear: bool,
    },

    /// Append a rule to the spec (mechanical — no validation beyond id/source/kind)
    Add {
        /// Rule id (must be unique within the spec)
        id: String,

        /// Local source path (directory or file, relative to root)
        #[arg(short = 'l', long = "local", value_name = "PATH", conflicts_with = "repo")]
        local: Option<std::path::PathBuf>,

        /// GitHub source: owner/repo[@ref][:path]
        #[arg(short = 'r', long = "repo", value_name = "SPEC", conflicts_with = "local")]
        repo: Option<String>,

        /// Kind is a single skill
        #[arg(
            long = "skill",
            conflicts_with_all = ["agent", "skill_set", "agent_set"]
        )]
        skill: bool,

        /// Kind is a single agent
        #[arg(
            long = "agent",
            conflicts_with_all = ["skill", "skill_set", "agent_set"]
        )]
        agent: bool,

        /// Kind is skill-set
        #[arg(
            long = "skill-set",
            visible_alias = "ss",
            conflicts_with_all = ["skill", "agent", "agent_set"]
        )]
        skill_set: bool,

        /// Kind is agent-set
        #[arg(
            long = "agent-set",
            visible_alias = "as",
            conflicts_with_all = ["skill", "agent", "skill_set"]
        )]
        agent_set: bool,

        /// Schema agent for the rule (required when spec has >1 agent)
        #[arg(short = 'g', long = "schema", value_name = "AGENT")]
        schema: Option<String>,

        /// Override frontmatter name (single kinds only)
        #[arg(
            long = "name",
            value_name = "NAME",
            conflicts_with_all = ["skill_set", "agent_set"]
        )]
        name: Option<String>,

        /// Override frontmatter description (single kinds only)
        #[arg(
            long = "description",
            value_name = "TEXT",
            conflicts_with_all = ["skill_set", "agent_set"]
        )]
        description: Option<String>,

        /// Override frontmatter allowed-tools (single kinds only, space-separated)
        #[arg(
            long = "allowed-tools",
            value_name = "TOOLS",
            conflicts_with_all = ["skill_set", "agent_set"]
        )]
        allowed_tools: Option<String>,

        /// Only include entries matching NAME (set kinds only, repeatable)
        #[arg(
            long = "include",
            value_name = "NAME",
            conflicts_with_all = ["skill", "agent", "exclude"]
        )]
        include: Vec<String>,

        /// Exclude entries matching NAME (set kinds only, repeatable)
        #[arg(
            long = "exclude",
            value_name = "NAME",
            conflicts_with_all = ["skill", "agent", "include"]
        )]
        exclude: Vec<String>,
    },
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    match cli.command {
        Command::Init { force, agent, no_detect } => init::exec(&root, force, agent, no_detect),
        Command::Sync { check, force, rule, adopt } => sync::exec(&root, check, force, rule, adopt),
        Command::Status { rule, verbose } => status::exec(&root, rule, verbose),
        Command::Own { path, rule_id, clear } => own::exec(&root, path, rule_id, clear),
        Command::Add {
            id,
            local,
            repo,
            skill,
            agent,
            skill_set,
            agent_set,
            schema,
            name,
            description,
            allowed_tools,
            include,
            exclude,
        } => add::exec(
            &root,
            add::AddOptions {
                id,
                local,
                repo,
                skill,
                agent,
                skill_set,
                agent_set,
                schema,
                name,
                description,
                allowed_tools,
                include,
                exclude,
            },
        ),
    }
}
