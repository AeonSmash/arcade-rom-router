-- CatVer.ini / Genre categories keyed by MAME set name.
CREATE TABLE set_categories (
    set_name     TEXT    NOT NULL COLLATE NOCASE PRIMARY KEY,
    category     TEXT    NOT NULL,
    source_path  TEXT,
    imported_at  TEXT    NOT NULL
);

CREATE INDEX idx_set_categories_category ON set_categories (category);
