-- schema-core.sql — shared SQL conventions for every source's processed DB
-- at sources/<name>/data/processed/processed.sqlite (decision: option B —
-- the DB holds ALL metadata; all bytes live as real files under
-- data/processed/asset/<kind>/..., referenced by assets.locator and
-- verified by sha256/size).
--
-- Two DISTINCT entity concepts — never conflated:
--   * words   : normalized Unicode I-words (pin: NFC). Used by IIIT, kaggle,
--               wikimedia, youtube.
--   * ngrams  : "some sort of n-gram (including n=1)" — sketch-engine's
--               I-entities. An n=1 ngram is NOT automatically a words row;
--               any correspondence is established by query, not by schema.
--
-- Matching convention — shared NAMES, not one shared table:
--   each matching source defines its own match table whose shared columns
--   are spelled identically (match_id, word_id, asset_id, method,
--   confidence, session_id, raw_ts, added_at, modified_at) plus its own
--   source-specific fields. EVERY match target is an asset_id: IIIT/kaggle
--   images, wikimedia audio, and youtube subtitle tracks are all files
--   stored under data/processed/asset/ (kind 'image' | 'audio' |
--   'subtitle-track'); youtube adds per-match fields (occurrences, cue
--   timings, ...) and a per-asset side table for track metadata.
--   sketch-engine has no match table at this stage.
--   Pair/triple word coverage is NEVER materialized — it is a self-join
--   query over the single-word matches (coverage-as-queries rule).
--
-- Provenance convention (every row that derives from acquired data):
--   * session_id : the PROCESS session that wrote the row (known at write
--                  time — the framework's current session id).
--   * raw_ts     : the data/raw/<ts>/ request folder the row derives from,
--                  when applicable. raw_ts IS the raw-path pointer; the raw
--                  SESSION that fetched it is derivable on demand via
--                  processing/session_for_raw_ts.py (this folder) — do not store
--                  it (would be redundant).
--
-- Timestamps: added_at set on INSERT (UTC), modified_at enforced by
-- trigger so no script can silently skip it. Every connection must run:
--   PRAGMA foreign_keys = ON;

-- ---------------------------------------------------------------- meta ---
CREATE TABLE IF NOT EXISTS meta (
    id             INTEGER PRIMARY KEY CHECK (id = 1),   -- single row
    source         TEXT NOT NULL,          -- e.g. 'wikimedia-audio'
    schema_version INTEGER NOT NULL,
    added_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now')),
    modified_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now'))
);

-- --------------------------------------------------------------- words ---
-- Normalized Unicode I-words (NFC applied by the processing code before
-- INSERT; SQLite cannot normalize).
CREATE TABLE IF NOT EXISTS words (
    word_id     INTEGER PRIMARY KEY,
    word        TEXT NOT NULL UNIQUE,      -- NFC-normalized Unicode
    session_id  TEXT NOT NULL,             -- process session that added it
    added_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now')),
    modified_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now'))
);
CREATE TRIGGER IF NOT EXISTS words_modified AFTER UPDATE ON words
BEGIN
    UPDATE words SET modified_at = strftime('%Y-%m-%d %H:%M:%S','now')
    WHERE word_id = NEW.word_id;
END;

-- -------------------------------------------------------------- assets ---
-- locator is relative to the source root (data/processed/asset/<kind>/...);
-- the FILE NAME is already the normalized Unicode form — normalizing raw
-- labels onto asset/ is the processing's job (youtube subtitle-track assets
-- are named by video id, as no single word names a track). raw_label
-- preserves what the raw source called it (e.g. kaggle folder 'kha' for క,
-- or the Commons filename).
CREATE TABLE IF NOT EXISTS assets (
    asset_id    INTEGER PRIMARY KEY,
    kind        TEXT NOT NULL,             -- 'image' | 'audio' | ...
    locator     TEXT NOT NULL UNIQUE,      -- relative path under source root
    sha256      TEXT NOT NULL,
    size        INTEGER NOT NULL,
    raw_label   TEXT,                      -- original label/filename in raw
    raw_ts      TEXT,                      -- data/raw/<ts> it derives from
    session_id  TEXT NOT NULL,
    added_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now')),
    modified_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now'))
    -- source-specific columns are ADDED per source (image: width/height/
    -- format/writer; audio: recorder/duration/license/author/commons_url)
);
CREATE TRIGGER IF NOT EXISTS assets_modified AFTER UPDATE ON assets
BEGIN
    UPDATE assets SET modified_at = strftime('%Y-%m-%d %H:%M:%S','now')
    WHERE asset_id = NEW.asset_id;
END;

-- ------------------------------------------------- match table template ---
-- Per-source; shown here as the file-backed variant (IIIT/kaggle/wikimedia):
--
-- CREATE TABLE matches (
--     match_id    INTEGER PRIMARY KEY,
--     word_id     INTEGER NOT NULL REFERENCES words(word_id),
--     asset_id    INTEGER NOT NULL REFERENCES assets(asset_id),
--     method      TEXT NOT NULL,          -- 'label-derived' | 'filename' ...
--     confidence  REAL NOT NULL,
--     session_id  TEXT NOT NULL,
--     raw_ts      TEXT,
--     added_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now')),
--     modified_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now')),
--     UNIQUE (word_id, asset_id)
-- );
--
-- youtube: same shape — asset_id points at the stored subtitle-track file
-- (kind 'subtitle-track') — plus occurrences and cue timing fields on the
-- match; a side table tracks(asset_id PRIMARY KEY REFERENCES assets, video
-- id, channel, title, language, human-vs-auto flag, duration) holds the
-- track metadata.
--
-- sketch-engine instead defines (no matches):
-- CREATE TABLE ngrams (
--     ngram_id    INTEGER PRIMARY KEY,
--     ngram       TEXT NOT NULL UNIQUE,   -- NFC-normalized n-gram text
--     n           INTEGER NOT NULL,       -- 1 | 2 | 3
--     freq        INTEGER,                -- raw exported columns kept as-is
--     rank        INTEGER,
--     per_million REAL,
--     export_csv  TEXT,                   -- which raw export it came from
--     raw_ts      TEXT,
--     session_id  TEXT NOT NULL,
--     added_at / modified_at as above (+ trigger)
-- );
