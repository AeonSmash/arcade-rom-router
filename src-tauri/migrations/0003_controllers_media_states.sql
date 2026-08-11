-- Phases 8–12: controllers, hotkey profiles, save states, media assets.

CREATE TABLE controllers (
    id           INTEGER PRIMARY KEY,
    device_id    TEXT    NOT NULL UNIQUE,
    display_name TEXT    NOT NULL,
    vendor_id    INTEGER,
    product_id   INTEGER,
    preset       TEXT    NOT NULL DEFAULT 'GENERIC',
    port         INTEGER NOT NULL DEFAULT 1,
    last_seen_at TEXT    NOT NULL,
    notes        TEXT
);

CREATE TABLE controller_bindings (
    id            INTEGER PRIMARY KEY,
    controller_id INTEGER REFERENCES controllers(id) ON DELETE CASCADE,
    scope         TEXT    NOT NULL DEFAULT 'UI',
    action        TEXT    NOT NULL,
    button_index  INTEGER,
    button_label  TEXT,
    axis_index    INTEGER,
    axis_direction TEXT,
    UNIQUE (controller_id, scope, action)
);

CREATE INDEX idx_controller_bindings_scope ON controller_bindings (scope);

CREATE TABLE hotkey_profiles (
    id              INTEGER PRIMARY KEY,
    name            TEXT    NOT NULL DEFAULT 'Default',
    enabled         INTEGER NOT NULL DEFAULT 0,
    exit_btn        INTEGER,
    exit_btn_label  TEXT,
    enable_btn      INTEGER,
    enable_btn_label TEXT,
    fragment_path   TEXT,
    verified        INTEGER NOT NULL DEFAULT 0,
    updated_at      TEXT    NOT NULL
);

INSERT INTO hotkey_profiles (name, enabled, updated_at)
VALUES ('Default', 0, datetime('now'));

CREATE TABLE save_states (
    id           INTEGER PRIMARY KEY,
    archive_id   INTEGER NOT NULL REFERENCES archives(id) ON DELETE CASCADE,
    slot         INTEGER NOT NULL,
    path         TEXT    NOT NULL,
    size_bytes   INTEGER NOT NULL DEFAULT 0,
    modified_at  TEXT,
    label        TEXT,
    thumbnail_path TEXT,
    is_entry     INTEGER NOT NULL DEFAULT 0,
    UNIQUE (archive_id, slot, is_entry)
);

CREATE INDEX idx_save_states_archive ON save_states (archive_id);

CREATE TABLE media_assets (
    id           INTEGER PRIMARY KEY,
    archive_id   INTEGER REFERENCES archives(id) ON DELETE CASCADE,
    set_name     TEXT,
    kind         TEXT    NOT NULL,
    path         TEXT    NOT NULL,
    source       TEXT    NOT NULL DEFAULT 'local',
    width        INTEGER,
    height       INTEGER,
    sha256       TEXT,
    fetched_at   TEXT    NOT NULL,
    UNIQUE (archive_id, kind, source)
);

CREATE INDEX idx_media_assets_archive ON media_assets (archive_id);
CREATE INDEX idx_media_assets_set ON media_assets (set_name);
CREATE INDEX idx_media_assets_kind ON media_assets (kind);
