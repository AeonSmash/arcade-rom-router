# Aeonic Arcadia Progress

## Current phase

Phase 12 — Controllers, Media, Save States (Favorites + EmuMovies sync options).

## Release

**v0.3.0** — Controllers, media, save states, favorites, EmuMovies sync, rename.
**v0.2.1** — RetroArch discovery hardening and FinalBurn Neo core binding fix.
**v0.2.0** — DAT import through RetroArch launch (Phases 2–7).

## Completed

### Phase 0 — Repository foundation

- [x] Tauri 2 desktop shell
- [x] React + TypeScript frontend
- [x] SQLite migrations, logging, settings, AppError
- [x] Test harness

### Phase 1 — ROM inventory

- [x] ROM root selection, read-only ZIP/CHD inventory, incremental scan
- [x] Source-safety tests

### Phase 2 — DAT import

- [x] Migration `0002_emulator_routing.sql`
- [x] XML DAT parser (Logiqx / MAME-style `game`/`machine`)
- [x] `dat_sources`, `machines`, `machine_roms`, `machine_disks`
- [x] Fingerprinting, duplicate refusal, activate/deactivate
- [x] DAT Manager UI

### Phase 3 — Matching

- [x] CRC/size candidate generation and classification
- [x] `match_results` persistence and confidence levels
- [x] Rematch on DAT import

### Phase 4 — Dependencies

- [x] Parent / BIOS / CHD resolution against inventory
- [x] Problem Center aggregates

### Phase 5 — Emulator profiles

- [x] Built-in RetroArch profile templates
- [x] RetroArch discovery + core scan
- [x] Health checks and DAT association
- [x] Emulator Manager UI

### Phase 6 — Router

- [x] Launchable route generation and preference modes
- [x] Per-game route override
- [x] Selected route on game detail

### Phase 7 — Launch

- [x] Safe `retroarch -L <core> <rom>` spawn (argument array)
- [x] Launch history + log file path
- [x] Play button gated on verified routes

### Phase 8 — Controller Center

- [x] Gamepad API detection, Xbox preset, live test panel
- [x] Configurable UI navigation bindings
- [x] Exit/ESC hotkey via Router-owned `--appendconfig` fragment

### Phase 10 — Save states

- [x] Read-only `retroarch.cfg` parsing for savestate paths
- [x] Slot index, labels, delete, resume-at-launch (`-e N`)

### Phase 11 — Local media

- [x] Artwork folders (box/screenshot/title/marquee/cabinet)
- [x] Set/parent/title resolution + asset protocol
- [x] Artwork-forward library grid

### Phase 12 — EmuMovies

- [x] MediaProvider trait + credential store (Windows Credential Manager)
- [x] Disabled by default
- [x] gamesdbase.com client (login / search / download) with Product/API key
- [x] Sync UI: media-type checkboxes + favorites-only or entire library

### Favorites (SPEC §53)

- [x] Toggle + Favorites library filter
- [x] Keyboard `F` when a game is selected
- [x] Star affordance on list/grid/detail

## In progress

Nothing.

## Next

- Controller Center / cabinet mode
- Wire gamepad FAVORITE action into library navigation
- Standalone MAME profile type (optional)
- Core Online Updater convenience (optional)
- Deeper packaging (split/merged) edge cases

## Decisions

- 2026-08-09: RetroArch + libretro cores is the primary runner; one MAME cannot
  cover mixed historical sets.
- 2026-08-09: A core without an active DAT cannot earn a high-confidence
  auto-route.
- 2026-08-09: Launch receives only `{ archiveId, routeId }`; paths are resolved
  in Rust and passed as an argument array.
- 2026-08-09: DAT import rematches the library immediately so the UI never shows
  a new definition with stale routes.

## Known issues

- Launch does not yet wait for process exit to record `exit_code` asynchronously.
- Problem Center lists counts but does not yet filter the library table on click.
- CHD presence uses path/name heuristics, not SHA-1 verification.
- Fake/help probe for RetroArch treats any process start with a code as OK.

## Tests

- 96 passing (unit + inventory + DAT/match + favorites/EmuMovies parse)
- 0 failing
- TypeScript `tsc --strict` clean
