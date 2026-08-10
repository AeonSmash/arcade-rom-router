-- Phase 0/1 subset of the schema described in SPEC.md section 29.
-- DAT, emulator, match, and route tables arrive in later phases.

CREATE TABLE rom_roots (
    id            INTEGER PRIMARY KEY,
    path          TEXT    NOT NULL UNIQUE,
    label         TEXT,
    recursive     INTEGER NOT NULL DEFAULT 1,
    enabled       INTEGER NOT NULL DEFAULT 1,
    read_only     INTEGER NOT NULL DEFAULT 1,
    watch_changes INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT    NOT NULL,
    last_scan_at  TEXT
);

CREATE TABLE archives (
    id                  INTEGER PRIMARY KEY,
    rom_root_id         INTEGER NOT NULL REFERENCES rom_roots(id) ON DELETE CASCADE,
    path                TEXT    NOT NULL UNIQUE,
    file_name           TEXT    NOT NULL,
    extension           TEXT    NOT NULL,
    size_bytes          INTEGER NOT NULL,
    modified_at         TEXT,
    quick_signature     TEXT    NOT NULL,
    sha256              TEXT,
    archive_state       TEXT    NOT NULL,
    member_count        INTEGER NOT NULL DEFAULT 0,
    unsafe_member_count INTEGER NOT NULL DEFAULT 0,
    error_detail        TEXT,
    last_scanned_at     TEXT    NOT NULL
);

CREATE INDEX idx_archives_rom_root ON archives (rom_root_id);
CREATE INDEX idx_archives_state ON archives (archive_state);
CREATE INDEX idx_archives_file_name ON archives (file_name);

CREATE TABLE archive_members (
    id                    INTEGER PRIMARY KEY,
    archive_id            INTEGER NOT NULL REFERENCES archives(id) ON DELETE CASCADE,
    member_name           TEXT    NOT NULL,
    size_bytes            INTEGER,
    compressed_size_bytes INTEGER,
    crc32                 TEXT,
    sha1                  TEXT,
    compression_method    TEXT,
    is_directory          INTEGER NOT NULL DEFAULT 0,
    name_is_safe          INTEGER NOT NULL DEFAULT 1
);

-- The CRC indexes exist now so the Phase 3 matching engine inherits them.
CREATE INDEX idx_members_archive ON archive_members (archive_id);
CREATE INDEX idx_members_crc32 ON archive_members (crc32);
CREATE INDEX idx_members_crc32_size ON archive_members (crc32, size_bytes);
CREATE INDEX idx_members_sha1 ON archive_members (sha1);

CREATE TABLE scan_jobs (
    id                INTEGER PRIMARY KEY,
    job_type          TEXT    NOT NULL,
    state             TEXT    NOT NULL,
    total_candidates  INTEGER NOT NULL DEFAULT 0,
    processed         INTEGER NOT NULL DEFAULT 0,
    inspected         INTEGER NOT NULL DEFAULT 0,
    reused_from_cache INTEGER NOT NULL DEFAULT 0,
    unreadable        INTEGER NOT NULL DEFAULT 0,
    removed           INTEGER NOT NULL DEFAULT 0,
    started_at        TEXT    NOT NULL,
    ended_at          TEXT,
    error_detail      TEXT
);

CREATE INDEX idx_scan_jobs_state ON scan_jobs (state);

CREATE TABLE settings (
    key        TEXT PRIMARY KEY,
    value_json TEXT NOT NULL
);
