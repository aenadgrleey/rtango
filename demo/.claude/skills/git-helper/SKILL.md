---
name: git-helper
description: Stage, commit, and push changes with a generated commit message.
allowed-tools: Read Bash Grep
---

# Git Helper

Workflow:

1. Run `git status` and `git diff` to inspect the working tree.
2. Draft a one-line commit message that explains the *why*, not the *what*.
3. Stage the relevant files explicitly (no `git add .`).
4. Commit and push to the current branch's upstream.
