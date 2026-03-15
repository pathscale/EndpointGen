# Changelog

All notable changes to this project will be documented in this file.
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


