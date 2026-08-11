# Aeonic Arcadia

Aeonic Arcadia is a local desktop library for mixed historical arcade ROM collections.

Instead of forcing every ROM through one MAME version, it inventories each archive, compares its ROM-chip checksums against emulator-specific DAT definitions, identifies missing parent/BIOS/CHD dependencies, and chooses a verified installed emulator route automatically.

The normal experience is simple:

1. Choose your arcade ROM folder.
2. Configure RetroArch and the arcade cores you use.
3. Import matching DAT definitions.
4. Scan.
5. Pick a game.
6. Press Play.

Your original ROM directory is read-only by default.

Aeonic Arcadia does not include or download copyrighted ROMs, BIOS files, or CHDs.

---

## Current status

See [PROGRESS.md](PROGRESS.md) for the detailed state, [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how the inventory engine is put together, and [SPEC.md](SPEC.md) for the full specification, which is the source of truth for this project.

What works today:

- Add one or more ROM folders, which are treated as read-only evidence.
- Scan them incrementally, with pause, resume, and cancel.
- Read every ZIP member's name, sizes, and CRC32 without decompressing anything.
- Index CHD files by path and size without hashing them.
- See damaged archives reported individually instead of failing the scan.
- Re-scan and have unchanged files skipped via a quick-signature cache.
- DAT import, matching, routing, RetroArch launch, favorites, controllers, media, and save states.

## Prerequisites

- Windows 10 or 11
- [Rust](https://rustup.rs) (stable, MSVC host toolchain)
- [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the "Desktop development with C++" workload
- [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/) (preinstalled on current Windows 11)
- Node.js 20 or newer

## Getting started

On Windows **Command Prompt**, change drive and folder in one step (`cd` alone does
not leave `C:`):

```bat
cd /d "F:\Arcade Emulation\aeonic-arcadia"
npm install
npm run tauri dev
```

In PowerShell, `cd` to that path is enough. Confirm the prompt shows the project
folder (and that `package.json` is there) before running npm.

Use the Aeonic Arcadia **desktop window**. Opening the Vite URL in a browser
will not work — there is no Tauri bridge there.

## Building a release

```bash
npm run tauri build
```

The optional EmuMovies provider sends an application product identifier with
each login. Supply it at build time through the `AEONIC_ARCADIA_EMUMOVIES_PRODUCT`
environment variable (or a local `.env`, which is gitignored). Never hardcode a
real key in `src-tauri/src/media/emumovies.rs`; the compiled-in default is a
placeholder only.

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

The Windows app identifier remains `com.arcaderomrouter.app` so existing libraries keep working after the rename from Arcade ROM Router.

## Legal

Aeonic Arcadia is software only. It ships no game content of any kind.

- You supply your own ROMs, BIOS files, CHDs, and DAT definition files. The application reads them where they already live and treats your ROM folders as read-only by default.
- No ROM search, download, or acquisition feature exists, and none will be added. See section 45 of [SPEC.md](SPEC.md): reporting a missing parent, BIOS set, or expected CRC is in scope; torrent search, ROM website search, and automatic acquisition of copyrighted ROMs are explicitly out of scope.
- Artwork from EmuMovies is fetched only with your own EmuMovies account, cached locally under `%APPDATA%`, and never redistributed by this project.
- MAME, RetroArch, EmuMovies, and LaunchBox are trademarks of their respective owners. This project is independent and is not affiliated with, endorsed by, or sponsored by any of them.

## License

MIT — see [LICENSE](LICENSE).

Repository: https://github.com/AeonSmash/aeonic-arcadia
