-- Phases 2–7: DAT definitions, matching, emulator profiles, routes, play history.

CREATE TABLE emulator_profiles (
    id               TEXT PRIMARY KEY,
    display_name     TEXT    NOT NULL,
    runner_type      TEXT    NOT NULL,
    executable_path  TEXT,
    core_path        TEXT,
    core_signature   TEXT,
    enabled          INTEGER NOT NULL DEFAULT 1,
    priority         INTEGER NOT NULL DEFAULT 50,
    settings_json    TEXT    NOT NULL DEFAULT '{}',
    last_health_check TEXT,
    health_state     TEXT    NOT NULL DEFAULT 'UNKNOWN'
);

CREATE TABLE dat_sources (
    id                   INTEGER PRIMARY KEY,
    emulator_profile_id  TEXT    NOT NULL REFERENCES emulator_profiles(id),
    display_name         TEXT    NOT NULL,
    source_type          TEXT    NOT NULL DEFAULT 'xml-dat',
    version              TEXT,
    path                 TEXT    NOT NULL,
    sha256               TEXT    NOT NULL,
    machine_count        INTEGER NOT NULL DEFAULT 0,
    rom_entry_count      INTEGER NOT NULL DEFAULT 0,
    disk_entry_count     INTEGER NOT NULL DEFAULT 0,
    imported_at          TEXT    NOT NULL,
    active               INTEGER NOT NULL DEFAULT 1,
    parser_version       INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX idx_dat_sources_profile ON dat_sources (emulator_profile_id);
CREATE INDEX idx_dat_sources_active ON dat_sources (active);

CREATE TABLE machines (
    id              INTEGER PRIMARY KEY,
    dat_source_id   INTEGER NOT NULL REFERENCES dat_sources(id) ON DELETE CASCADE,
    set_name        TEXT    NOT NULL,
    description     TEXT,
    year            TEXT,
    manufacturer    TEXT,
    clone_of        TEXT,
    rom_of          TEXT,
    is_bios         INTEGER NOT NULL DEFAULT 0,
    runnable        INTEGER,
    metadata_json   TEXT
);

CREATE UNIQUE INDEX idx_machines_dat_set ON machines (dat_source_id, set_name);
CREATE INDEX idx_machines_set_name ON machines (set_name);
CREATE INDEX idx_machines_clone_of ON machines (clone_of);
CREATE INDEX idx_machines_rom_of ON machines (rom_of);

CREATE TABLE machine_roms (
    id           INTEGER PRIMARY KEY,
    machine_id   INTEGER NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
    name         TEXT    NOT NULL,
    size_bytes   INTEGER,
    crc32        TEXT,
    sha1         TEXT,
    status       TEXT,
    optional     INTEGER NOT NULL DEFAULT 0,
    merge_name   TEXT,
    bios_name    TEXT,
    region       TEXT
);

CREATE INDEX idx_machine_roms_machine ON machine_roms (machine_id);
CREATE INDEX idx_machine_roms_crc32 ON machine_roms (crc32);
CREATE INDEX idx_machine_roms_crc32_size ON machine_roms (crc32, size_bytes);
CREATE INDEX idx_machine_roms_sha1 ON machine_roms (sha1);

CREATE TABLE machine_disks (
    id          INTEGER PRIMARY KEY,
    machine_id  INTEGER NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
    name        TEXT    NOT NULL,
    sha1        TEXT,
    status      TEXT,
    optional    INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_machine_disks_machine ON machine_disks (machine_id);
CREATE INDEX idx_machine_disks_sha1 ON machine_disks (sha1);

CREATE TABLE match_results (
    id                   INTEGER PRIMARY KEY,
    archive_id           INTEGER NOT NULL REFERENCES archives(id) ON DELETE CASCADE,
    machine_id           INTEGER NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
    emulator_profile_id  TEXT    NOT NULL REFERENCES emulator_profiles(id),
    dat_source_id        INTEGER NOT NULL REFERENCES dat_sources(id) ON DELETE CASCADE,
    state                TEXT    NOT NULL,
    confidence           TEXT    NOT NULL,
    matched_required     INTEGER NOT NULL DEFAULT 0,
    missing_required     INTEGER NOT NULL DEFAULT 0,
    wrong_required       INTEGER NOT NULL DEFAULT 0,
    score                REAL    NOT NULL DEFAULT 0,
    evidence_json        TEXT    NOT NULL DEFAULT '[]',
    created_at           TEXT    NOT NULL
);

CREATE INDEX idx_match_results_archive ON match_results (archive_id);
CREATE INDEX idx_match_results_machine ON match_results (machine_id);
CREATE INDEX idx_match_results_state ON match_results (state);
CREATE INDEX idx_match_results_profile ON match_results (emulator_profile_id);

CREATE TABLE routes (
    id                   INTEGER PRIMARY KEY,
    archive_id           INTEGER NOT NULL REFERENCES archives(id) ON DELETE CASCADE,
    machine_id           INTEGER NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
    emulator_profile_id  TEXT    NOT NULL REFERENCES emulator_profiles(id),
    match_result_id      INTEGER NOT NULL REFERENCES match_results(id) ON DELETE CASCADE,
    is_selected          INTEGER NOT NULL DEFAULT 0,
    selection_reason     TEXT,
    user_override        INTEGER NOT NULL DEFAULT 0,
    launchable           INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_routes_archive ON routes (archive_id);
CREATE INDEX idx_routes_selected ON routes (archive_id, is_selected);

CREATE TABLE play_history (
    id          INTEGER PRIMARY KEY,
    archive_id  INTEGER NOT NULL REFERENCES archives(id) ON DELETE CASCADE,
    route_id    INTEGER REFERENCES routes(id) ON DELETE SET NULL,
    started_at  TEXT    NOT NULL,
    ended_at    TEXT,
    exit_code   INTEGER,
    user_result TEXT,
    log_path    TEXT
);

CREATE INDEX idx_play_history_archive ON play_history (archive_id);

CREATE TABLE favorites (
    archive_id INTEGER PRIMARY KEY REFERENCES archives(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL
);

-- Built-in RetroArch arcade profile templates (SPEC.md §7).
INSERT INTO emulator_profiles (id, display_name, runner_type, enabled, priority, health_state) VALUES
    ('fbneo', 'FinalBurn Neo', 'retroarch', 1, 80, 'UNKNOWN'),
    ('mame2003plus', 'MAME 2003-Plus', 'retroarch', 1, 70, 'UNKNOWN'),
    ('mame2003', 'MAME 2003', 'retroarch', 1, 60, 'UNKNOWN'),
    ('mame2010', 'MAME 2010', 'retroarch', 1, 55, 'UNKNOWN'),
    ('mame2015', 'MAME 2015', 'retroarch', 1, 50, 'UNKNOWN'),
    ('mame2016', 'MAME 2016', 'retroarch', 1, 45, 'UNKNOWN'),
    ('mame_current', 'MAME Current', 'retroarch', 1, 40, 'UNKNOWN');
