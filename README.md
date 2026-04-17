# rtango

Package manager for AI-agent skills, agents, and configuration files.

Author a skill once — in whichever agent's format you prefer — and `rtango` renders and syncs copies to every other agent's native layout (frontmatter, permission schema, target paths) from a single source of truth.

## Why

Modern projects juggle several coding agents (Claude Code, Copilot, Codex, OpenCode, Pi, …). Each one stores skills, agents, and instruction files in different paths with different frontmatter and different permission vocabularies. Hand-porting and keeping them in sync is tedious and error-prone.

`rtango` treats this like a package manager: declare rules in `.rtango/spec.yaml`, run `rtango sync`, and every agent gets an up-to-date rendering.

## Install

```sh
cargo install --path .
```

Binary name is `rtango`.

## Quick start

```sh
# scan repo, detect agents, write .rtango/spec.yaml + lock.yaml
rtango init

# append a rule
rtango add my-skill --local .claude/skills/my-skill --skill

# preview the plan
rtango status

# write files, update the lock
rtango sync
```

## Commands

| Command  | Purpose |
| -------- | ------- |
| `init`   | Detect installed agents and bootstrap `.rtango/spec.yaml` + `.rtango/lock.yaml`. |
| `sync`   | Fetch sources, render per-agent outputs, write files, update the lock. `--check` (CI dry-run), `--adopt` (absorb existing files on first sync), `--force` (override `on_target_modified: fail`), `--rule <id>` (single rule). |
| `status` | Preview the sync plan without writing. `--verbose` shows up-to-date items too. |
| `own`    | Record or clear a manual ownership decision when multiple rules target the same path. |
| `add`    | Mechanically append a rule to the spec. Kinds: `--skill`, `--agent`, `--skill-set`, `--agent-set`, `--system`. |

## Core concepts

- **Rule** — a `{source, schema_agent, kind}` declaration in `spec.yaml`. Source is a local path or `github: owner/repo@ref:path`. `schema_agent` names the authoritative agent whose format the source is written in.
- **Kinds** — `skill`, `skill-set`, `agent`, `agent-set`, `system` (root-level instruction files like `AGENTS.md` / `CLAUDE.md`).
- **Rendering** — for each agent in `spec.agents`, rtango rewrites frontmatter and permission tokens into that agent's native schema and writes to its canonical path. The `schema_agent`'s own target is auto-skipped when source and target paths collide.
- **Lock** — `.rtango/lock.yaml` records what was written, content hashes, and ownership decisions. Changes to target files detected outside rtango are caught by the `on_target_modified` policy (`fail` / `overwrite` / `skip`).

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

See [AGENTS.md](AGENTS.md) for contributor and agent-facing context.
