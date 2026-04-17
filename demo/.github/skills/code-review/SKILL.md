---
name: code-review
description: Walk the diff against main and surface risks, missing tests, and naming issues.
allowed-tools: read grep glob shell
---

# Code Review

Walk the diff against `main` and surface:

- Missing tests for new public surface
- Type errors, unhandled `Result`s, unwraps in non-test code
- Naming inconsistencies and dead code
- Public API changes without a CHANGELOG entry

End with a short summary of the most important findings.
