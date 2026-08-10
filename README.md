# Arcade ROM Router

Arcade ROM Router is a local desktop library for mixed historical arcade ROM collections.

Instead of forcing every ROM through one MAME version, it inventories each archive, compares its ROM-chip checksums against emulator-specific DAT definitions, identifies missing parent/BIOS/CHD dependencies, and chooses a verified installed emulator route automatically.

The normal experience is simple:

1. Choose your arcade ROM folder.
2. Configure RetroArch and the arcade cores you use.
3. Import matching DAT definitions.
4. Scan.
5. Pick a game.
6. Press Play.

Your original ROM directory is read-only by default.

Arcade ROM Router does not include or download copyrighted ROMs, BIOS files, or CHDs.

---

## Current status

**Phase 1 — ROM Inventory.** The application scans a folder and produces a trustworthy inventory of every archive and its ROM-chip checksums. It does not yet identify games, resolve dependencies, choose emulator routes, or launch anything; those are Phases 2 through 7.

See [PROGRESS.md](PROGRESS.md) for the detailed state, [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how the inventory engine is put together, and [SPEC.md](SPEC.md) for the full specification, which is the source of truth for this project.

What works today:

- Add one or more ROM folders, which are treated as read-only evidence.
- Scan them incrementally, with pause, resume, and cancel.
- Read every ZIP member's name, sizes, and CRC32 without decompressing anything.
- Index CHD files by path and size without hashing them.
- See damaged archives reported individually instead of failing the scan.
- Re-scan and have unchanged files skipped via a quick-signature cache.

## Prerequisites

- Windows 10 or 11
- [Rust](https://rustup.rs) (stable, MSVC host toolchain)
- [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the "Desktop development with C++" workload
- [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/) (preinstalled on current Windows 11)
- Node.js 20 or newer

## Getting started

```bash
npm install
npm run tauri dev
```

## Building a release

```bash
npm run tauri build
```

## Tests

```bash
# Rust: unit and integration tests, including the source-safety check
cd src-tauri && cargo test

# TypeScript type checking
npm run typecheck
```

Test fixtures are synthetic ZIP archives generated at test time from deterministic pseudo-random bytes. No copyrighted ROM data is used, committed, or required.

## Project layout

```text
src/          React + TypeScript frontend
src-tauri/    Rust backend (scanner, archive reader, database, commands)
  migrations/ SQLite schema migrations
  tests/      Integration tests and the synthetic fixture generator
SPEC.md       Full product and implementation specification (source of truth)
PROGRESS.md   Living implementation status
```

## Data locations

Everything the application stores is local and lives under the per-user application data directory:

```text
%APPDATA%\com.arcaderomrouter.app\
  library.db     SQLite inventory and settings
  logs\          Rotating diagnostic logs
```

Nothing is written to your ROM folders.

## License

MIT. See [LICENSE](LICENSE).
