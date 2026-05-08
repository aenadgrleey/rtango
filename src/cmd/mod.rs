pub mod add;
pub mod init;
pub mod own;
pub mod status;
pub mod sync;
pub mod wander;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "rtango",
    version,
    about = "Package manager for agent skills and configuration files"
)]
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

        /// Skip auto-detection, create an empty spec skeleton
        #[arg(short, long)]
        no_detect: bool,

        /// Keep generated targets in a managed .gitignore block
        #[arg(long)]
        gitignore_targets: bool,
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

        /// Skip GitHub rules whose fetch fails and continue with the rest
        #[arg(long)]
        ignore_fetch_failures: bool,
    },

    /// Show sync plan without writing anything
    Status {
        /// Only show a single rule
        #[arg(short, long, value_name = "ID")]
        rule: Option<String>,

        /// Show up-to-date items too
        #[arg(short, long)]
        verbose: bool,

        /// Skip GitHub rules whose fetch fails and continue with the rest
        #[arg(long)]
        ignore_fetch_failures: bool,
    },

    /// Run init + sync in-memory: render target files without creating `.rtango/`
    Wander {
        /// Additional target agent to render for (repeatable)
        #[arg(short = 't', long = "target", value_name = "AGENT")]
        targets: Vec<String>,

        /// Skip GitHub rules whose fetch fails and continue with the rest
        #[arg(long)]
        ignore_fetch_failures: bool,
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

        /// Local source path (directory or file, relative to root).
        /// Combine with --collection-kind/--col to treat a local directory as a collection.
        #[arg(
            short = 'l',
            long = "local",
            value_name = "PATH",
            conflicts_with = "repo"
        )]
        local: Option<std::path::PathBuf>,

        /// GitHub source: owner/repo[@ref][:path].
        /// Combine with --collection-kind/--col to treat a GitHub repo as a collection.
        #[arg(
            short = 'r',
            long = "repo",
            value_name = "SPEC",
            conflicts_with = "local"
        )]
        repo: Option<String>,

        /// Kind is a single skill
        #[arg(
            long = "skill",
            conflicts_with_all = ["agent", "skill_set", "agent_set", "system", "collection_kind"]
        )]
        skill: bool,

        /// Kind is a single agent
        #[arg(
            long = "agent",
            conflicts_with_all = ["skill", "skill_set", "agent_set", "system", "collection_kind"]
        )]
        agent: bool,

        /// Kind is skill-set
        #[arg(
            long = "skill-set",
            visible_alias = "ss",
            conflicts_with_all = ["skill", "agent", "agent_set", "system", "collection_kind"]
        )]
        skill_set: bool,

        /// Kind is agent-set
        #[arg(
            long = "agent-set",
            visible_alias = "as",
            conflicts_with_all = ["skill", "agent", "skill_set", "system", "collection_kind"]
        )]
        agent_set: bool,

        /// Kind is a system instruction file (CLAUDE.md / AGENTS.md / etc.)
        #[arg(
            long = "system",
            conflicts_with_all = [
                "skill", "agent", "skill_set", "agent_set", "collection_kind",
                "name", "description", "allowed_tools", "include", "exclude",
            ]
        )]
        system: bool,

        /// Kind is a remote rtango collection (imports rules from a remote spec.yaml)
        #[arg(
            long = "collection-kind",
            visible_alias = "col",
            conflicts_with_all = [
                "skill", "agent", "skill_set", "agent_set", "system",
                "name", "description", "allowed_tools",
            ]
        )]
        collection_kind: bool,

        /// Schema agent for the rule (required when spec has >1 agent)
        /// For collections, overrides the schema_agent for all imported rules.
        #[arg(short = 'g', long = "schema", value_name = "AGENT")]
        schema: Option<String>,

        /// Override frontmatter name (single kinds only)
        #[arg(
            long = "name",
            value_name = "NAME",
            conflicts_with_all = ["skill_set", "agent_set", "collection_kind"]
        )]
        name: Option<String>,

        /// Override frontmatter description (single kinds only)
        #[arg(
            long = "description",
            value_name = "TEXT",
            conflicts_with_all = ["skill_set", "agent_set", "collection_kind"]
        )]
        description: Option<String>,

        /// Override frontmatter allowed-tools (single kinds only, space-separated)
        #[arg(
            long = "allowed-tools",
            value_name = "TOOLS",
            conflicts_with_all = ["skill_set", "agent_set", "collection_kind"]
        )]
        allowed_tools: Option<String>,

        /// Only include entries matching NAME (set and collection kinds, repeatable)
        #[arg(
            long = "include",
            value_name = "NAME",
            conflicts_with_all = ["skill", "agent", "exclude"]
        )]
        include: Vec<String>,

        /// Exclude entries matching NAME (set and collection kinds, repeatable)
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
        Command::Init {
            force,
            no_detect,
            gitignore_targets,
        } => init::exec(&root, force, no_detect, gitignore_targets),
        Command::Sync {
            check,
            force,
            rule,
            adopt,
            ignore_fetch_failures,
        } => sync::exec_with_options(&root, check, force, rule, adopt, ignore_fetch_failures),
        Command::Status {
            rule,
            verbose,
            ignore_fetch_failures,
        } => status::exec_with_options(&root, rule, verbose, ignore_fetch_failures),
        Command::Wander {
            targets,
            ignore_fetch_failures,
        } => wander::exec_with_options(&root, targets, ignore_fetch_failures),
        Command::Own {
            path,
            rule_id,
            clear,
        } => own::exec(&root, path, rule_id, clear),
        Command::Add {
            id,
            local,
            repo,
            skill,
            agent,
            skill_set,
            agent_set,
            system,
            collection_kind,
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
                system,
                collection_kind,
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

pub(crate) fn print_skipped_github_fetches(skipped: &[crate::engine::SkippedGithubFetch]) {
    for skipped_fetch in skipped {
        eprintln!(
            "warning: skipped GitHub rule '{}' ({})\n  {}",
            skipped_fetch.rule_id, skipped_fetch.source, skipped_fetch.message
        );
    }
}
