---
description: How to use rtango — the package manager for AI-agent skills, agents, and system instructions. Covers init, sync, status, conflict resolution, and common workflows.
name: rtango
---

# rtango Skill

This skill teaches AI agents how to use `rtango` to manage and sync skills, agents, and instruction files across multiple coding agents (Claude Code, GitHub Copilot, Codex, OpenCode, Pi).

## What rtango Does

rtango is a package manager for AI-agent configuration. You author a skill or instruction file **once** (in whichever agent's format you prefer), declare it in `.rtango/spec.yaml`, and rtango renders and syncs copies to every other agent's native layout — rewriting frontmatter, permissions, and target paths automatically.

## Core Concepts

- **spec.yaml** (`.rtango/spec.yaml`) — declares agents, rules, and defaults. This is the source of truth for what gets synced.
- **lock.yaml** (`.rtango/lock.yaml`) — records what was last written, content hashes, and ownership decisions. Never hand-edit; regenerate with `rtango sync`.
- **Rule** — a `{id, source, schema_agent, kind}` declaration. Source can be a local path or a GitHub ref (`github: owner/repo@ref:path`).
- **Kind** — one of: `skill`, `skill-set`, `agent`, `agent-set`, `system` (root-level instruction files like `AGENTS.md` / `CLAUDE.md`).
- **schema_agent** — the agent whose native format the source file is written in. rtango reads frontmatter and permissions using this agent's parser.

## Commands

### `rtango init`

Scan the project, detect installed agents, and bootstrap `.rtango/spec.yaml` + `.rtango/lock.yaml`.

```sh
rtango init           # detect agents and create spec
rtango init --force   # overwrite existing spec
rtango init --no-detect  # create empty skeleton without auto-detection
```

Run this once when setting up rtango in a new project.

### `rtango add`

Append a rule to the spec. This is a mechanical helper — it does not validate that the source exists or sync anything.

```sh
# Add a local skill (single file or directory)
rtango add my-skill --local .claude/skills/my-skill --skill

# Add a local agent
rtango add my-agent --local .claude/agents/my-agent --agent

# Add a skill-set (directory containing multiple skills)
rtango add my-skills --local .github/skills/ --skill-set

# Add an agent-set
rtango add my-agents --local .github/agents/ --agent-set

# Add a system instruction file
rtango add instructions --local AGENTS.md --system

# Add from GitHub
rtango add upstream-skill --repo "github: owner/repo@abc123:path/to/skill" --skill

# Specify schema agent when spec has multiple agents
rtango add my-skill --local .github/skills/my-skill --skill --schema copilot

# Override frontmatter fields
rtango add my-skill --local src/skill.md --skill --name "My Skill" --description "Does things"
```

After `add`, run `rtango sync` to materialize the files.

### `rtango status`

Preview what sync would do **without writing anything**. Use this before every sync to verify the plan.

```sh
rtango status              # show creates, updates, conflicts, orphans
rtango status --verbose    # also show up-to-date items
rtango status --rule my-skill  # only show one rule
```

The output labels each target file:
- **create** — target doesn't exist yet; will be created.
- **update** — source changed since last sync; target will be overwritten.
- **conflict** — target was modified outside rtango and policy blocks overwrite (see Conflicts below).
- **orphan** — target exists in the lock but its rule/source was removed from the spec.
- **up-to-date** — nothing to do (shown only with `--verbose`).

### `rtango sync`

Execute the sync plan: fetch remote sources, render per-agent outputs, write files, and update the lock.

```sh
rtango sync                # normal sync
rtango sync --check        # CI dry-run: exit 1 if out of sync
rtango sync --force        # override on_target_modified: fail
rtango sync --adopt        # absorb existing target files on first sync
rtango sync --rule my-skill  # only sync one rule
```

**`--check`** is designed for CI pipelines. It runs the full plan but does not write files. Exit code 0 means everything is in sync; exit code 1 means something is out of date.

**`--adopt`** is useful when you already have target files that match what rtango would generate. Instead of treating them as conflicts, adopt records them in the lock as if they had been synced.

**`--force`** overwrites files even when the `on_target_modified` policy is `fail`. Use with care — it discards any local edits made since the last sync.

### `rtango own`

Record or clear a manual ownership decision when multiple rules claim the same target path.

```sh
rtango own .claude/skills/my-skill rule-a     # assign ownership to rule-a
rtango own .claude/skills/my-skill --clear    # remove ownership decision
```

Ownership decisions are persisted in `.rtango/lock.yaml` and respected on future syncs.

### `rtango wander`

Run init + sync in-memory without creating `.rtango/` files. Auto-detects sources from agent folders and renders to additional agents specified via `--target`. Useful for one-off rendering.

```sh
rtango wander                          # detect and render
rtango wander --target copilot         # also render for copilot
rtango wander --target copilot --target pi  # render for multiple agents
```

## Common Workflows

### Starting from Scratch

```sh
# 1. Initialize
rtango init

# 2. Add rules
rtango add my-skill --local .claude/skills/my-skill --skill
rtango add my-agents --local .github/agents/ --agent-set

# 3. Preview
rtango status

# 4. Sync
rtango sync
```

### Adding a Remote Skill from GitHub

```sh
# Add a skill from a specific commit
rtango add code-review --repo "github: addyosmani/agent-skills@abc123:skills/code-review" --skill

# Preview and sync
rtango status
rtango sync
```

### CI Gate

```sh
# In your CI pipeline:
rtango sync --check
# Exit 0 = in sync, exit 1 = drift detected
```

### Checking What Changed

```sh
# Before syncing, always preview:
rtango status --verbose

# Then sync when satisfied:
rtango sync
```

## Conflict Resolution

### Path Ambiguity (Multiple Rules Target the Same Path)

When two or more rules would write to the same target file, rtango flags an **ambiguity**. During interactive `sync`, rtango prompts you to pick which rule owns the path. The decision is saved to the lock.

To resolve non-interactively:

```sh
rtango own .claude/skills/shared-skill my-rule-id
```

### Target Modified Outside rtango

When a target file has been edited by hand since the last sync, rtango detects this via content hash mismatch. The default policy (`on_target_modified: fail`) blocks the sync to protect your edits.

**Resolution options:**

1. **Keep your edits, update the source** — copy your changes back to the source file and re-sync.
2. **Discard your edits** — run `rtango sync --force` to overwrite the target.
3. **Change the policy** — set `on_target_modified: overwrite` or `skip` in the rule or spec defaults:
   ```yaml
   defaults:
     on_target_modified: fail  # global default
   rules:
     - id: my-skill
       on_target_modified: overwrite  # per-rule override
   ```
4. **Adopt the current file** — run `rtango sync --adopt` to accept the current file as-is and update the lock without overwriting.

### Orphaned Files

When a rule is removed from the spec but its target file still exists, `rtango status` shows it as **orphan**. Running `rtango sync` will remove the orphaned file. If you want to keep it, either re-add the rule or move the file out of the managed path before syncing.

## spec.yaml Reference

```yaml
version: 1

agents:
  - claude-code
  - copilot

defaults:
  on_target_modified: fail  # fail | overwrite | skip

rules:
  # Local skill-set
  - id: my-skills
    source: .github/skills/
    schema_agent: copilot
    kind: skill-set

  # Local single skill
  - id: my-skill
    source: .claude/skills/my-skill
    schema_agent: claude-code
    kind: skill
    on_target_modified: overwrite  # per-rule override

  # Remote skill from GitHub (pinned to a commit hash)
  - id: upstream-review
    source:
      github: owner/repo
      ref: abc123def456
      path: skills/code-review
    schema_agent: claude-code
    kind: skill

  # System instruction file
  - id: project-instructions
    source: AGENTS.md
    schema_agent: claude-code
    kind: system
```

## Tips

- **Always run `rtango status` before `rtango sync`** to preview changes.
- **Pin GitHub sources to commit hashes** (not branches) for reproducible builds.
- **The `schema_agent`'s own target is auto-skipped** when source and target paths are the same — a rule targeting copilot with `schema_agent: copilot` is a no-op for copilot but still renders for other agents.
- **Never hand-edit `.rtango/lock.yaml`** — regenerate it with `rtango sync`.
- **Use `--rule` to sync a single rule** when iterating on one skill without touching others.
- **System-kind rules** (like `AGENTS.md`) have no frontmatter — the source content is written verbatim.
