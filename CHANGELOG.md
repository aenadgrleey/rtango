# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.1](https://github.com/aenadgrleey/rtango/compare/v0.3.0...v0.3.1) - 2026-05-06

### Fixed

- format collection rules and tests

## [0.3.0](https://github.com/aenadgrleey/rtango/compare/v0.2.0...v0.3.0) - 2026-05-06

### Added

- add collection kind for importing rules from external rtango specs

### Added

- `kind: collection` rule — point at any local path or GitHub repo that contains a `.rtango/spec.yaml` and all its rules are imported and rendered into the local project's agents
- `rtango add --col`/`--collection-kind` flag to append collection rules from the CLI
- `include`/`exclude` filters on collection rules to selectively import rule ids
- `schema_override` on collection rules to force a different parser for all imported rules
- Imported rules are namespaced in the lock as `<collection-id>/<rule-id>` to avoid collisions with local rules
- Collection-vs-collection path conflicts are resolved through the existing interactive prompt and `rtango own` ownership mechanism
- User-defined collection/skill/skill-set rules that produce a `rtango`-named skill correctly suppress the built-in rtango skill

## [0.2.0](https://github.com/aenadgrleey/rtango/compare/v0.1.0...v0.2.0) - 2026-05-05

### Added

- add built-in rtango skill auto-injected on every sync

### Other

- install via `cargo install rtango`
