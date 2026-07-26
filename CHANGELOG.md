# Changelog

All notable changes to this project will be documented in this file.
## [1.13.1] - 2026-07-26

### Documentation

- Document every flag, and correct the version-compatibility rule

## [1.13.0] - 2026-07-26

### Features

- [**breaking**] Make the OpenAPI and AsyncAPI documents opt-in

## [1.12.0] - 2026-07-25

### Features

- Add the OpenAPI 3.1 emitter
- Add the AsyncAPI 3.0 emitter
- Wire the emitters into the build and --check

## [1.11.0] - 2026-07-25

### Features

- Add --check, and make generation deterministic

### Styling

- Fix rustfmt drift, and add the OpenAPI golden fixture

## [1.10.1] - 2026-07-25

### Documentation

- Adopt the AGENTS.md agent standard + guardrails
- Scope the force-push rule to the default branch

## [1.10.0] - 2026-07-25

### Features

- Fail generation on empty endpoint and enum-variant descriptions
- Extend empty-description validation to error codes

### Miscellaneous Tasks

- Move to endpoint-libs 2.0 (lockstep)
- Depend on published endpoint-libs 2.0.0-alpha.1

## [1.9.0] - 2026-07-17

### Bug Fixes

- Patch endpoint-libs via git branch so CI can resolve it
- Point endpoint-libs patch at main (MCP PRs merged, branch deleted)
- Resolve clippy lints failing CI (large_enum_variant, collapsible_if)

### Features

- Emit type_registry() and MCP tools docs for endpoint-libs MCP surface

### Miscellaneous Tasks

- Require endpoint-libs 1.9.0 from crates.io, drop temporary patch

## [1.5.1] - 2026-03-28

### Miscellaneous Tasks

- Migrate to ubicloud build machine
- Update endpoint-libs to 1.3.5

## [1.5.0] - 2026-03-15

### Features

- Add parent element configuration for EnumList

### Miscellaneous Tasks

- Update example project

## [1.4.0] - 2026-03-15

### Features

- Add configurable JSON Schema generation via [schemars](https://crates.io/crates/schemars) codegen derive and import

### Miscellaneous Tasks

- Formatting and release fixes

## [1.3.4] - 2026-03-06

### Bug Fixes

- Rename comment->description
- Remove DataTable support and fix documentation generation

### Features

- Remove unused dependencies
- Improve enum variant sorting in generated rust code
- Ignore non-ron files and return early if no files are found
- Allow specifying whether an endpoint is FE facing or not
- Support snake case field names with configurable conversion setting
- Rename types to make more sense
- Remove ws feature from endpoint-libs dep as we don't actually need it

### Miscellaneous Tasks

- Add version flag to cli

## [0.1.5] - 2024-10-08

### Features

- Bugfix clippy
- Fix clippy

### Fixes

- Make sure paths exist before attempting to use them


