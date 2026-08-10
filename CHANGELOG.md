# Changelog

All notable changes to this project are recorded here. This project follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] — 2026-08-09

Initial release. Phase 0 repository foundation and Phase 1 ROM inventory.

### Added

- Phase 0 — Repository foundation: Tauri 2 desktop shell with a React and
  TypeScript frontend, a Rust backend, embedded SQLite migrations, structured
  logging to rotating files, a JSON settings store, and a test harness.
- Phase 1 — ROM inventory: ROM root selection, read-only ZIP enumeration,
  per-member filename/size/CRC32 indexing, CHD indexing by path and size,
  cancellable and pausable scan jobs, an incremental quick-signature cache,
  and the archive inventory table.
- Clear notice when the Vite frontend is opened in a browser instead of the
  Tauri desktop window, so missing `invoke` is explained rather than shown as
  an unexpected internal error.

### Security

- ROM roots are opened through a single read-only filesystem module that
  exposes no write, rename, or delete operations.
- ZIP members are read from the central directory only; no member data is ever
  decompressed, and the `zip` dependency is built without compression codecs.
- Archive member names that attempt path traversal are recorded as evidence and
  refused as paths.
- Dynamic SQL is limited to placeholder counts and filter shape; every value is
  bound.

[Unreleased]: https://github.com/AeonSmash/arcade-rom-router/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/AeonSmash/arcade-rom-router/releases/tag/v0.1.0
