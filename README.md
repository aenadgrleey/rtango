# rtango

Package manager for AI-agent skills and system instruction files.

Author a skill once — in whichever agent's format you prefer — and `rtango` renders and syncs copies to every other agent's native layout (frontmatter, permission schema, target paths) from a single source of truth.

## Why

Modern projects juggle several coding agents (Claude Code, Copilot, Cursor, Codex, OpenCode, Pi, Hermes, …). Each one stores skills and instruction files in different paths with different frontmatter and different permission vocabularies. Hand-porting and keeping them in sync is tedious and error-prone.

`rtango` treats this like a package manager: declare rules in `.rtango/spec.yaml`, run `rtango sync`, and every agent gets an up-to-date rendering.

## Install

From [crates.io](https://crates.io/crates/rtango):

```sh
cargo install rtango
```

Or from a local checkout:

```sh
cargo install --path .
```

Binary name is `rtango`.

## Quick start

```sh
# scan repo, detect agents, write .rtango/spec.yaml + lock.yaml
rtango init

# append a local skill rule
rtango add my-skill --local .claude/skills/my-skill --skill

# import all skills from another repo (local path)
rtango add team-tools --local ../shared-tools --col

# import from a GitHub repo (pinned to a commit)
rtango add team-tools --repo owner/shared-tools@abc123 --col

# preview the plan
rtango status

# write files, update the lock
rtango sync

# sync the default user-level registry spec
rtango global-sync
```

## GitHub auth and fetch failures

For GitHub-backed rules, rtango first uses `RTANGO_GITHUB_TOKEN` or `GITHUB_TOKEN` if either is set.
If those are absent, rtango also tries `gh auth token` from an existing GitHub CLI login.
If a GitHub fetch hits auth/rate-limit issues during a command, rtango can lazily ask to run
`gh auth login`, then retry immediately.

If you want the rest of the spec to continue when a GitHub fetch fails, use:

```sh
rtango sync --ignore-fetch-failures
rtango status --ignore-fetch-failures
rtango wander --ignore-fetch-failures
```

## Commands

| Command  | Purpose |
| -------- | ------- |
| `init`   | Detect installed agents and bootstrap `.rtango/spec.yaml` + `.rtango/lock.yaml`. `--gitignore-targets` adds a managed `.gitignore` block for generated targets. |
| `sync`   | Fetch sources, render per-agent outputs, write files, update the lock. `--check` (CI dry-run), `--adopt` (absorb existing files on first sync), `--force` (override `on_target_modified: fail`), `--rule <id>` (single rule), `--ignore-fetch-failures` (skip GitHub rules whose fetch fails). When `defaults.gitignore_targets: true`, sync also maintains a managed `.gitignore` block. |
| `global-sync` | Render `~/.rtango/spec.yaml` into global agent registries. Use `--spec PATH` as an override; targets can come from the spec, per-rule `targets`, or compatibility CLI arguments. Supports `--check`, `--force`, `--prune`, and `--lock`. Alias: `sync-global`. |
| `status` | Preview the sync plan without writing. `--verbose` shows up-to-date items too. `--ignore-fetch-failures` skips GitHub rules whose fetch fails. |
| `own`    | Record or clear a manual ownership decision when multiple rules target the same path. |
| `add`    | Mechanically append a rule to the spec. Kinds: `--skill`, `--skill-set`, `--system`, `--collection-kind`/`--col`. |

## Core concepts

- **Rule** — a `{source, schema_agent, kind}` declaration in `spec.yaml`. Source is a local path or `github: owner/repo@ref:path`. `schema_agent` names the authoritative agent whose format the source is written in.
- **Kinds** — `skill`, `skill-set`, `system` (root-level instruction files like `AGENTS.md` / `CLAUDE.md` / `SOUL.md`), `collection` (import rules from another repo's `.rtango/spec.yaml`).
- **Rendering** — for each agent in `spec.agents`, rtango rewrites frontmatter and permission tokens into that agent's native schema and writes to its canonical path. The `schema_agent`'s own target is auto-skipped when source and target paths collide.
- **System files** — a `kind: system` rule copies one markdown source verbatim to each selected agent's instruction-file convention. The current target paths are:

  | Agent | Target |
  | --- | --- |
  | Claude Code | `CLAUDE.md` |
  | Codex, Cursor, OpenCode, Pi, Hermes | `AGENTS.md` |
  | GitHub Copilot | `.github/copilot-instructions.md` |
  | Plain | `system/AGENTS.md` |

- **Lock** — `.rtango/lock.yaml` records what was written, content hashes, and ownership decisions. Changes to target files detected outside rtango are caught by the `on_target_modified` policy (`fail` / `overwrite` / `skip`).
- **Managed `.gitignore`** — optionally keep projected targets in a dedicated rtango block. Skill projections are ignored precisely as leaf directories (for example `.pi/skills/reviewer/`), while system projections are ignored as exact files; broad roots like `.pi/` are never ignored.

## Global registry sync

`global-sync` is a user-level projection mode. Its canonical spec is
`~/.rtango/spec.yaml`; pass `--spec PATH` only when using another registry:

```sh
rtango global-sync --spec ~/configs/team-spec.yaml
```

If the default spec is missing or empty, the command reports that state and
suggests `rtango add --global`. Relative sources resolve beside the spec, and
the sidecar lock defaults to `spec.lock.yaml` for `spec.yaml`. The legacy
`~/.config/rtango/global.yaml` is detected with a migration warning.
Existing targets are protected by `on_target_modified`; use `--force` when a
manual edit should be replaced. Files removed from the spec are retained by
default and are deleted only with `--prune`.

The global spec keeps ordinary settings such as `defaults.on_target_modified`,
rule `include`/`exclude`, frontmatter overrides, and `schema_agent`. Only these
project-specific settings are ignored and reported:

- `defaults.gitignore_targets` — no project `.gitignore` is touched;
- `spec.local.yaml` — the selected global spec is self-contained;
- project target paths, ownership decisions, and built-in skills.

`spec.agents` is the default target set. A rule can override it with explicit
targets, including several Codex profiles:

```yaml
version: 1
agents: [claude-code, codex]
rules:
  - id: review
    source: sources/review
    schema_agent: claude-code
    kind: skill
    targets:
      - agent: codex
        home: ~/.codex/personal
      - agent: codex
        home: ~/.codex/work
```

Add scaffolds directly to the default spec:

```sh
rtango add --global reviewer --skill --target codex=~/.codex/work
```

Without `--local` or `--repo`, this command creates editable sources under
`~/.rtango/sources/`.

Global skills use the documented shared Agent Skills directory for Codex,
`~/.agents/skills/`; project-scoped Codex output remains `.codex/skills`.
Global file-backed instruction targets are `$CLAUDE_CONFIG_DIR/CLAUDE.md` (default `~/.claude/CLAUDE.md`),
`$CODEX_HOME/AGENTS.md`, `~/.copilot/copilot-instructions.md`,
`$XDG_CONFIG_HOME/opencode/AGENTS.md`, `~/.pi/agent/AGENTS.md`, and `$HERMES_HOME/SOUL.md`.
The command honors `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `COPILOT_HOME`, `XDG_CONFIG_HOME`, and `HERMES_HOME` from
its process environment, and an explicit rule `targets[].home` redirects each profile directly (for example `claude-code` `home: ~/.claude-t3/personal` writes to `~/.claude-t3/personal/{CLAUDE.md,skills/}`, and `hermes` `home: ~/.hermes/profiles/pachca` writes to `~/.hermes/profiles/pachca/{SOUL.md,skills-rtango/}`).
Cursor User Rules are settings-backed rather than a documented global file, so
global system rules for `cursor` are rejected; global skills remain supported.

The complete global registry matrix is:

| Agent | Skills | System instructions |
| --- | --- | --- |
| Claude Code | `$CLAUDE_CONFIG_DIR/skills/` (default `~/.claude/skills/`) | `$CLAUDE_CONFIG_DIR/CLAUDE.md` |
| Codex | `~/.agents/skills/` (or `<target.home>/skills/`) | `$CODEX_HOME/AGENTS.md` (or existing `AGENTS.override.md`) |
| Copilot CLI | `$COPILOT_HOME/skills/` | `$COPILOT_HOME/copilot-instructions.md` |
| Cursor | `~/.cursor/skills/` (or `<target.home>/.cursor/skills/`) | Not file-backed; rejected |
| OpenCode | `$XDG_CONFIG_HOME/opencode/skills/` | `$XDG_CONFIG_HOME/opencode/AGENTS.md` |
| Pi | `~/.pi/agent/skills/` | `~/.pi/agent/AGENTS.md` |
| Hermes | `$HERMES_HOME/skills-rtango/` (mount via `skills.external_dirs`) | `$HERMES_HOME/SOUL.md` |

The paths above show defaults; `CODEX_HOME`, `COPILOT_HOME`, and
`XDG_CONFIG_HOME` can redirect the corresponding registries. A rule-level
`targets[].home` is the declarative way to write to several independent
profiles in one sync. Global sync treats an existing untracked target as a
conflict; `--force` is required to replace it. `--adopt` is project-only.

### Local overrides

An optional `.rtango/spec.local.yaml` overlays the shared `.rtango/spec.yaml`. It is intended for machine- or developer-specific configuration and should normally be added to `.gitignore`.

```yaml
version: 1

# Replace the main agent list or individual defaults when present.
agents: [cursor, codex]
defaults:
  on_target_modified: overwrite

# Remove rules declared by the main spec from the effective configuration.
exclude:
  - team-only-skill

# A matching id replaces the main rule; a new id adds a local-only rule.
rules:
  - id: reviewer
    source: .cursor/skills/reviewer
    schema_agent: cursor
    kind: skill
```

Rule replacement is by exact `id` and keeps the main rule's ordering. Excluded IDs must exist in the main spec, and the same ID cannot be both excluded and overridden. `rtango add` always edits only `.rtango/spec.yaml`; it never copies local overrides into the shared file. Remote collection specs do not load their own `spec.local.yaml` files.

## Repository signals for agents

If a repo uses rtango, make that obvious so coding agents know they should sync managed skills instead of hand-editing generated copies.

Recommended signals to commit:
- `.rtango/spec.yaml` — the strongest signal that rtango manages agent files here.
- `.rtango/lock.yaml` — shows the repo is actively synced.
- A short note in `AGENTS.md`, `CLAUDE.md`, or `README.md` telling agents to use `rtango status` / `rtango sync` after changing shared skills or instructions.
- Optionally `defaults.gitignore_targets: true` so projected outputs stay ignored consistently across machines.

A good agent-facing note is:

```md
This repo uses rtango to manage agent skills/instructions.
After changing shared agent files or `.rtango/spec.yaml`, run:
- `rtango status`
- `rtango sync`
```

## Supported agents

Claude Code, GitHub Copilot, Cursor, Codex, OpenCode, Pi, Hermes. Each has its own parser, writer, and permission mapper under `src/agent/`.

## Layout

```
src/
  cmd/       # subcommand handlers
  spec/      # spec.yaml + lock.yaml types and IO
  agent/     # per-agent parse/write/frontmatter/permission
  engine/    # expand → fetch → plan → execute pipeline
demo/        # worked example with .rtango/ and sample skills
tests/       # integration tests
```

## Collections

A **collection** points at another repo's `.rtango/spec.yaml` and imports its declared rules into your project. The source is just a local path or a GitHub ref — the `kind: collection` is what triggers the import.

```sh
# Local sibling repo
rtango add team-skills --local ../team-skills --col

# GitHub repo, pinned to a commit
rtango add team-skills --repo org/team-skills@abc123 --col

# Import only specific rules
rtango add team-skills --repo org/team-skills@abc123 --col --include code-review
```

Imported rules are namespaced in the lock as `<collection-id>/<rule-id>`. When two collections produce the same target path, rtango detects the conflict and prompts during `sync` — or use `rtango own <path> <rule-id>` to resolve it non-interactively.

See [AGENTS.md](AGENTS.md) for contributor and agent-facing context.
