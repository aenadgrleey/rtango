# rtango

Package manager for AI-agent skills, agents, and configuration files.

Author a skill once — in whichever agent's format you prefer — and `rtango` renders and syncs copies to every other agent's native layout (frontmatter, permission schema, target paths) from a single source of truth.

## Why

Modern projects juggle several coding agents (Claude Code, Copilot, Codex, OpenCode, Pi, …). Each one stores skills, agents, and instruction files in different paths with different frontmatter and different permission vocabularies. Hand-porting and keeping them in sync is tedious and error-prone.

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
```

## Commands

| Command  | Purpose |
| -------- | ------- |
| `init`   | Detect installed agents and bootstrap `.rtango/spec.yaml` + `.rtango/lock.yaml`. `--gitignore-targets` adds a managed `.gitignore` block for generated targets. |
| `sync`   | Fetch sources, render per-agent outputs, write files, update the lock. `--check` (CI dry-run), `--adopt` (absorb existing files on first sync), `--force` (override `on_target_modified: fail`), `--rule <id>` (single rule). When `defaults.gitignore_targets: true`, sync also maintains a managed `.gitignore` block. |
| `status` | Preview the sync plan without writing. `--verbose` shows up-to-date items too. |
| `own`    | Record or clear a manual ownership decision when multiple rules target the same path. |
| `add`    | Mechanically append a rule to the spec. Kinds: `--skill`, `--agent`, `--skill-set`, `--agent-set`, `--system`, `--collection-kind`/`--col`. |

## Core concepts

- **Rule** — a `{source, schema_agent, kind}` declaration in `spec.yaml`. Source is a local path or `github: owner/repo@ref:path`. `schema_agent` names the authoritative agent whose format the source is written in.
- **Kinds** — `skill`, `skill-set`, `agent`, `agent-set`, `system` (root-level instruction files like `AGENTS.md` / `CLAUDE.md`), `collection` (import rules from another repo's `.rtango/spec.yaml`).
- **Rendering** — for each agent in `spec.agents`, rtango rewrites frontmatter and permission tokens into that agent's native schema and writes to its canonical path. The `schema_agent`'s own target is auto-skipped when source and target paths collide.
- **Lock** — `.rtango/lock.yaml` records what was written, content hashes, and ownership decisions. Changes to target files detected outside rtango are caught by the `on_target_modified` policy (`fail` / `overwrite` / `skip`).
- **Managed `.gitignore`** — optionally keep user-managed projected targets in a dedicated rtango block. Skill projections are ignored precisely as leaf directories (for example `.pi/skills/reviewer/`), while agent/system projections are ignored as exact files; broad roots like `.pi/` are never ignored.

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

Claude Code, GitHub Copilot, Codex, OpenCode, Pi. Each has its own parser, writer, and permission mapper under `src/agent/`.

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
