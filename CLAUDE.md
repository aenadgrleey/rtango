# AGENTS.md

Context for AI agents working on the `rtango` codebase.

## What this repo is

`rtango` is a Rust CLI that acts as a package manager for AI-agent skills, agents, and system instruction files. A single source file is declared in `.rtango/spec.yaml` and rendered/synced into every supported agent's native layout (paths, frontmatter, permission vocabulary).

See `README.md` for the user-facing overview.

## Layout

```
src/
  main.rs              # thin entry, parses Cli and calls cmd::run
  lib.rs               # re-exports cmd
  cmd/                 # subcommand handlers (init, sync, status, own, add)
    mod.rs             # Cli + Command enum (clap derive) and dispatch
  spec/                # spec.yaml + lock.yaml data model and IO
  agent/               # per-agent support
    impls/             # claude_code.rs, copilot.rs, codex.rs, opencode.rs, pi.rs
    parse.rs / write.rs / frontmatter.rs / permission.rs / detect.rs
  engine/              # orchestration pipeline: expand → fetch → plan → execute
tests/                 # integration tests (run end-to-end against tempdirs)
demo/                  # worked example — real spec.yaml, lock.yaml, sample skills
```

## Pipeline mental model

`rtango sync` runs a fixed pipeline:

1. **expand** — spec rules → concrete `ExpandedItem`s (one item per source file × target agent).
2. **fetch** — resolve remote sources (GitHub refs) into the cache.
3. **parse** — the `schema_agent`'s parser reads frontmatter and permissions into a canonical representation.
4. **plan** — diff against `.rtango/lock.yaml`; classify each target as create / update / unchanged / conflict.
5. **execute** — write files, update the lock.

`status` stops after `plan`. `sync --check` exits non-zero if the plan is non-empty (CI gate).

## Core types

- `Spec` (`src/spec/spec.rs`) — `version`, `agents`, `defaults`, `rules`.
- `Rule` — `id`, `source`, `schema_agent`, `kind`, optional `include`/`exclude`, optional frontmatter overrides, optional per-rule `on_target_modified`.
- `Source` — local path or `github: owner/repo[@ref][:path]`.
- `Kind` — `skill`, `skill-set`, `agent`, `agent-set`, `system`.
- `Lock` — tracks rendered outputs, content hashes, ownership decisions.
- **Canonical permission set**: Read, Write, Edit, Shell, Grep, Glob, WebFetch, WebSearch, NotebookRead, NotebookEdit, TodoRead, TodoWrite, ListDir, Other. Each agent impl maps this set to its native tokens.

## Adding a new agent

1. Create `src/agent/impls/<agent>.rs` implementing the agent parser/writer/frontmatter/permission traits.
2. Register it in `src/agent/mod.rs`'s dispatcher.
3. Teach `init` auto-detection in `src/cmd/init.rs` if the agent has a well-known folder layout.
4. Add an integration test in `tests/`.

## Dev commands

```sh
cargo build                 # debug build
cargo build --release       # release binary in target/release/rtango
cargo test                  # unit + integration tests
cargo run -- <args>         # e.g. cargo run -- status
cargo clippy --all-targets  # lint
cargo fmt                   # format
```

For manual end-to-end testing, `cd demo` and run `../target/debug/rtango status`.

## Conventions

- Prefer editing existing modules over adding new top-level files.
- Integration tests use `tempfile::tempdir()` and a real project root — do not mock the filesystem.
- Lockfile ordering is deterministic; if you touch serialization, re-run `cargo test` and confirm snapshots still round-trip.
- When adding a CLI flag, keep the clap doc comments (`///`) concise — they become the `--help` output.
- Error handling uses `anyhow` at CLI boundaries and `thiserror` for typed module errors; follow the surrounding file's style.
- Don't hand-edit `.rtango/lock.yaml` in fixtures — regenerate it with `rtango sync` and commit the result.

## Gotchas

- The `schema_agent`'s own target path is auto-skipped when it equals the source path — a rule that names copilot as `schema_agent` and targets copilot's folder is a no-op for copilot but still renders for the other agents.
- A rule touching the same target path as another rule triggers an **ambiguity**; resolve with `rtango own <path> <rule-id>` which writes the decision into the lock.
- `on_target_modified: fail` (default) refuses to overwrite a file the user edited since the last sync. Use `--force` only when you genuinely want to discard local edits.
- System-kind rules (`CLAUDE.md`, `AGENTS.md`, …) live at the repo root and do not have frontmatter; the parser treats them as opaque content.
