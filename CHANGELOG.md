# Changelog

All notable changes to this project are recorded here. This project follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] — 2026-08-10

Controllers, media, save states, favorites, and product rename.

### Changed

- Product renamed to **Aeonic Arcadia** (repo: https://github.com/AeonSmash/aeonic-arcadia).
  Windows app identifier stays `com.arcaderomrouter.app` so existing AppData
  libraries continue to load. EmuMovies credentials migrate from the old
  keyring service name on next read; re-save if needed.
- App phase string advances to Phase 12 — Controllers, Media, Save States.

### Added

- Phase 8 — Controller Center: Gamepad API detection, Xbox navigation defaults,
  live button test panel, and UI navigation toggle.
- Phase 8b — Router-owned RetroArch hotkey fragment for Exit/ESC (and optional
  hold modifier) applied only via `--appendconfig`; never writes `retroarch.cfg`.
- Phase 10 — Save state browser with labels, delete, and resume-at-launch.
- Phase 11 — Local artwork folders, media cache, asset protocol scope, and
  artwork-forward library grid.
- Phase 12 — EmuMovies provider scaffold (credentials in OS keyring, disabled
  by default; live API gated on developer access).
- Favorites: toggle (detail button + `F` key), library sidebar category, and
  `isFavorite` on archive rows.
- EmuMovies sync: site username/password only (LaunchBox-style; app supplies
  product id), gamesdbase.com login/search/download, Media settings checkboxes
  for media types, Favorites-only or entire-library scope, and a sync summary.

## [0.2.1] — 2026-08-10

### Fixed

- FinalBurn Neo no longer binds to FB Alpha CPS/NeoGeo split cores
  (`fbalpha2012_cps1_…`); discovery prefers `fbneo_libretro` when present.
- MAME 2003 discovery ignores the Midway-only fork so it cannot steal the main
  2003 profile.
- Emulator Manager reports a clear configuration error when RetroArch is not
  found, instead of silently leaving profiles unconfigured.

### Changed

- RetroArch auto-detect also checks portable layouts such as
  `C:\RetroArch-Win64` and related drive roots.
- README documents Windows `cd /d` so npm commands run from the project folder.

## [0.2.0] — 2026-08-10

RetroArch multi-core routing: DAT import through launch.

### Added

- Phase 2 — DAT import: XML parser (Logiqx / MAME-style), fingerprinting, DAT
  Manager UI, and `machines` / `machine_roms` / `machine_disks` tables.
- Phase 3 — CRC/size matching with confidence levels and compatibility states;
  rematch on DAT import.
- Phase 4 — Parent / BIOS / CHD dependency resolution and Problem Center.
- Phase 5 — RetroArch discovery, arcade core scan, profile health checks, DAT
  association, and Emulator Manager UI.
- Phase 6 — Launchable route ranking, preference modes, and per-game overrides.
- Phase 7 — Safe RetroArch `-L <core> <rom>` launch (argument array) with play
  history logging.
- Library setup banner when archives are indexed but no DAT is active; clearer
  DAT empty-state guidance and Problem Center counting for profiles missing DATs.

### Changed

- App phase string advances to Phase 7 — Emulator Routing.

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

[Unreleased]: https://github.com/AeonSmash/aeonic-arcadia/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/AeonSmash/aeonic-arcadia/releases/tag/v0.3.0
[0.2.1]: https://github.com/AeonSmash/aeonic-arcadia/releases/tag/v0.2.1
[0.2.0]: https://github.com/AeonSmash/aeonic-arcadia/releases/tag/v0.2.0
[0.1.0]: https://github.com/AeonSmash/aeonic-arcadia/releases/tag/v0.1.0
