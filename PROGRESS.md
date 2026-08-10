# Arcade ROM Router Progress

## Current phase

Phase 1 — ROM Inventory (complete), released as **v0.1.0**. Stopped before Phase 2 — DAT Import, as instructed by SPEC.md section 85.

## Completed

### Phase 0 — Repository foundation

- [x] Tauri 2 desktop shell (Tauri 2.11, Rust 1.97, MSVC toolchain)
- [x] React 19 + TypeScript 5.8 + Vite 7 frontend
- [x] SQLite via `sqlx` 0.9 with WAL and `PRAGMA foreign_keys = ON`
- [x] Embedded migrations (`0001_initial.sql`), applied at startup
- [x] Structured logging with `tracing`, daily rotating files, and an in-memory
      INFO buffer for the Diagnostics view
- [x] Typed JSON settings store
- [x] Categorized `AppError` model (SPEC.md section 46)
- [x] Test harness with synthetic fixtures

### Phase 1 — ROM inventory

- [x] ROM root selection with a native folder picker
- [x] Read-only filesystem module with no write surface
- [x] ZIP enumeration honouring the section 11 content policy
- [x] Archive member inspection: name, sizes, CRC32, compression method
- [x] CHD indexing by path and size, without hashing
- [x] Path-traversal member names recorded and flagged
- [x] Damaged archives recorded as `ARCHIVE_UNREADABLE` without aborting a scan
- [x] Incremental quick-signature cache
- [x] Cancellable and pausable scan jobs with batched transactions
- [x] Throttled scan progress events
- [x] Virtualized archive inventory table with an evidence detail panel
- [x] Design tokens and always-visible focus rings

## In progress

Nothing. Phase 1 is complete and the work is stopped at the Phase 2 boundary.

## Next

- [ ] XML DAT parser
- [ ] `machines`, `machine_roms`, `machine_disks` tables
- [ ] DAT source fingerprinting and duplicate detection
- [ ] DAT Manager UI
- [ ] Synthetic DAT test suite

## Decisions

- 2026-08-09: The spec lives at `SPEC.md` in the repository root. Keeping a
  second copy under `documents/` would let the two drift, and SPEC.md section 6
  and the bootstrap prompt both reference the root path.
- 2026-08-09: Source ROM roots remain read-only by default, enforced
  structurally. All ROM-root access goes through `archive::fs_readonly`, which
  opens handles with write access explicitly disabled and exposes no create,
  write, rename, or delete function. There is no code path that can mutate a
  ROM root.
- 2026-08-09: Member CRC32 values are read from the ZIP central directory and
  never by decompressing member data. The `zip` dependency is built with
  `default-features = false`, so no compression codec is compiled into the
  binary at all and the guarantee is structural rather than conventional.
- 2026-08-09: `quick_signature` is the SHA-256 of path, size, and modification
  time with explicit field separators. Hashing keeps the stored value a fixed
  width regardless of path length, and the separators prevent adjacent fields
  from being confused.
- 2026-08-09: SHA-256 of a whole file is computed only in Deep Verify mode,
  never during a normal or incremental scan (SPEC.md section 12.2).
- 2026-08-09: Scans run on a bounded worker pool sized
  `clamp(logical_cores - 1, 1, 8)` so a scan leaves the machine usable.
- 2026-08-09: Results are committed in transactions of 100 archives. An archive
  row and its member rows are always written together, so a cancelled scan
  leaves complete records rather than partial ones.
- 2026-08-09: Pruning of records for missing files happens only after a root has
  been walked in full. A cancelled pass never deletes records for files it did
  not reach.
- 2026-08-09: One scan at a time. Concurrent scans of overlapping roots would
  race on the same archive rows for no user benefit.
- 2026-08-09: Progress events are throttled to roughly ten per second, with an
  unthrottled event immediately after each batch commit so the interface learns
  about newly durable rows without waiting.
- 2026-08-09: Scan jobs left `RUNNING` by a crash are reconciled to `FAILED` at
  startup, so the interface never shows a scan with no process behind it.
- 2026-08-09: Zustand and TanStack Query are deferred. SPEC.md section 5.2 warns
  against adding state libraries gratuitously, and Phase 1 state is a handful of
  values in one component tree.
- 2026-08-09: Dynamic SQL is limited to placeholder counts and the shape of the
  WHERE clause; every value is bound. `sqlx` 0.9 requires such strings to be
  wrapped in `AssertSqlSafe`, and each use is annotated with what was audited.
- 2026-08-09: Archive rows whose stored state string is unrecognised are
  surfaced as unreadable rather than silently treated as indexed, so a database
  written by a newer build cannot mislabel evidence.

## Deviations from SPEC.md

1. **Rust integration tests live in `src-tauri/tests/`, not the repository-root
   `tests/` directory shown in section 6.** Cargo only discovers integration
   tests inside the crate, so a root-level `tests/` directory could not be run
   by `cargo test`. The `tests/fixtures/` subtree in section 6 is also absent
   because fixtures are generated at test time rather than committed, which
   section 60.2 requires.

2. **The Phase 0/1 schema implements only a subset of section 29.** The
   `rom_roots`, `archives`, `archive_members`, `settings`, and `scan_jobs`
   tables exist. DAT, emulator, match, and route tables are deliberately left to
   later phases. The `archive_members` CRC indexes are created now so the Phase
   3 matcher inherits them.

3. **`archives` carries three columns section 29 does not name:**
   `member_count`, `unsafe_member_count`, and `error_detail`. The first two let
   the library table render without a join per row; the third preserves the
   exact parse error for a damaged archive, which section 12.4 requires be kept
   for diagnostics.

4. **`ArchiveState` uses `DISK_IMAGE_INDEXED` for CHD files.** Section 14's
   compatibility states describe post-matching outcomes, which do not exist yet.
   Phase 1 needs to distinguish "read in full" from "recorded by metadata only",
   and a CHD is the latter by design (section 41).

5. **Scan control includes pause and resume.** Section 68 names the rescan
   modes and the plan's UI calls for pause alongside cancel; `PAUSED` already
   appears in the section 47 job states, so the commands were added to match.

6. **`.7z` is not scanned.** Section 11 places it in Phase 2.

## Known issues

- The library table fetches the first 1,000 matching rows rather than paging as
  the user scrolls. Row rendering is already virtualized, so this only matters
  above 1,000 archives; server-side paging is wired into the query layer
  (`limit`/`offset`) and needs only a frontend change.
- The Diagnostics view is not built yet. The backend already collects INFO-level
  history and exposes `get_diagnostics`, but no screen renders it.
- Settings have no UI. Values are readable and writable through commands, and
  the scan worker count is honoured, but nothing surfaces them.
- Scan progress is persisted to `scan_jobs` at start and finish only. A crash
  mid-scan therefore loses the intermediate counters, though the archive rows
  themselves are already durable.
- Filesystem watching (`watch_changes`) is stored on each root but not acted on.

## Tests

- 69 passing (54 unit, 15 integration)
- 0 failing
- TypeScript type checking passes with `strict`, `noUnusedLocals`, and
  `noUnusedParameters`

Source safety is covered by `scanning_never_modifies_a_single_source_file`,
which SHA-256s every fixture before and after full, incremental, and deep-verify
scans and asserts that nothing in the ROM folder changed.
