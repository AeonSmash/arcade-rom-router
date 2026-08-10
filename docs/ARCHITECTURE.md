# Architecture — Phases 0 and 1

This describes the system as it exists today: a read-only ROM inventory engine.
Nothing here matches games, resolves dependencies, chooses emulator routes, or
launches anything. Those are Phases 2 through 7.

The rationale behind individual choices lives in the Decisions section of
[PROGRESS.md](../PROGRESS.md). This document covers the shape of the system.

## The one rule everything else serves

The user's ROM collection is evidence. The application reads it and never
changes it. Every structural decision below exists to make that guarantee hold
by construction rather than by discipline.

## Layers

```text
React + TypeScript
        │  invoke(command)  /  listen("scan://progress")
        ▼
commands/          narrow, typed, one function per user intent
        ▼
scanner/           orchestration: enumerate, compare, dispatch, commit
        ▼
archive/           inspection of one file
  fs_readonly      the only door into a ROM root
  zip_reader       central-directory metadata, no decompression
        ▼
db/                sqlx over SQLite, WAL, foreign keys on
```

Each layer only calls downward. `archive` knows nothing about jobs or progress;
`db` knows nothing about ZIP files.

## The read-only guarantee

`archive::fs_readonly` is the only module that opens a file inside a ROM root.
It offers exactly four operations: open for reading, read metadata, hash, and
format bytes as hex. Handles are opened with `write(false)`, `append(false)`,
`create(false)`, `create_new(false)`, and `truncate(false)` stated explicitly,
so the intent survives future edits rather than resting on defaults.

There is no function in that module that creates, writes, renames, moves,
truncates, or deletes. Because no other module reaches into a ROM root, the
absence of those functions means no code path can mutate the collection.

This is verified two ways: a unit test asserts that a handle returned by
`open_read` rejects writes, and an integration test SHA-256s an entire synthetic
ROM folder before and after full, incremental, and deep-verify scans.

## Reading a ZIP without decompressing it

An arcade ROM set stores one chip dump per archive member, and the CRC32 of each
member is the evidence that identifies it. That CRC is already recorded in the
ZIP central directory, so `zip_reader` iterates entries with `by_index_raw`,
which hands back an entry without constructing a decompressor.

The `zip` dependency is declared with `default-features = false`. No compression
codec is compiled into the binary, so "we never inflate member data" is a
property of the build rather than a promise about the code.

A malformed archive is a finding, not a failure: `archive::inspect` never
returns an error. An unparseable file becomes an `ARCHIVE_UNREADABLE` record
carrying the exact parse error, and the scan continues. A problem with one entry
inside an otherwise valid archive becomes a warning attached to that archive.

Member names that try to escape the archive are recorded and flagged rather than
dropped, so the evidence is preserved. Nothing is extracted today, so such a
name cannot cause harm; the rule is in force before any future feature gains the
ability to write files.

## How a scan runs

```text
enumerate every root        → a real total before any work starts
compare quick signatures    → unchanged files are skipped entirely
inspect on a worker pool    → bounded, blocking work off the async runtime
commit in batches of 100    → archive and its members written together
emit progress               → throttled, plus one event per commit
prune missing files         → only after a root is walked in full
```

`quick_signature` is the SHA-256 of path, size, and modification time. A Quick
scan compares it against the stored value and skips matches; a Full scan ignores
the cache; Deep Verify additionally computes a SHA-256 of each file.

Inspection is blocking I/O, so it runs on `spawn_blocking` through a `JoinSet`
that is kept exactly `worker_count` tasks deep. Completed results flow back to
the single async task that owns the database, which batches them into
transactions. SQLite writes are serialized anyway, so one writer avoids lock
contention without costing throughput.

## Cancellation and pause

`JobControl` pairs a `CancellationToken` with a paused flag and a `Notify`.
Cancellation is checked once per completed archive, after progress has been
reported, so the last state the interface sees reflects everything committed.
Pausing blocks the dispatch of new work while letting in-flight archives finish;
cancelling also releases a paused job so it cannot be stranded.

Two invariants make partial results trustworthy:

- An archive row and its member rows are written in the same transaction, so a
  persisted archive is never missing the members that were read from it.
- Records for files that no longer exist are pruned only after a root has been
  walked in full, so a cancelled pass never deletes records for files it simply
  did not reach.

A subsequent Quick scan reuses everything already committed and inspects only
the remainder, which makes cancel-and-resume cheap.

## Errors at the boundary

`AppError` classifies every failure into one of the SPEC.md section 46
categories and serializes as `{ category, title, message, technicalDetails }`.
The message is plain language written for the user; the raw Rust error text is
confined to `technicalDetails`, which the interface shows only behind a
disclosure. A unit test asserts that raw parse text cannot leak into the primary
message.

## Command boundary

The frontend names an intent and the backend resolves every path itself. There
is no `run_process`, no `read_any_file`, and no `write_any_file`. The window's
capability grants exactly two plugin permissions: opening a folder picker and
opening a URL.

Progress is pushed over the `scan://progress` Tauri event. The frontend also
asks for current status on mount, so reloading the window during a scan
reconnects to it instead of appearing idle.

## Frontend

State is plain React hooks. Phase 1 has a handful of values in one component
tree, and SPEC.md section 5.2 warns against adding state libraries before the
complexity justifies them.

Colour, spacing, typography, and motion come from CSS custom properties in
`src/styles/tokens.css`. Components reference tokens rather than literals, so a
later theme changes one file. Reduced-motion and increased-contrast preferences
are honoured there too.

Status is never carried by colour alone: each chip pairs a glyph with a written
label. Focus rings are defined once globally and are always visible, which also
prepares the ground for the controller navigation described in section 56.

The archive table virtualizes rows via `@tanstack/react-virtual`, so a
collection of several thousand archives scrolls without rendering every row.

## Testing

Fixtures are synthetic. `tests/common` builds ZIP archives at test time from
deterministic xorshift bytes and computes each member's expected CRC32 with an
independent implementation, so the assertions cross-check the scanner rather
than restate its output. No copyrighted data is used, and nothing binary is
committed.

The suite covers filename normalization, member enumeration, CRC parsing,
cache hit and miss, traversal rejection, extension filtering, damaged archives,
deletion pruning, cancellation boundaries, resume, and source safety.
