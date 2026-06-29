-- Kith schema v1. Append-only: never edit this file after it is applied;
-- add a new NNNN_*.sql migration instead. Timestamps are ISO-8601 TEXT.

CREATE TABLE individuals (
    id          INTEGER PRIMARY KEY,
    sex         TEXT,                         -- 'M','F','X','U' (extensible)
    given_name  TEXT,
    surname     TEXT,
    name_prefix TEXT,
    name_suffix TEXT,
    nickname    TEXT,
    living      INTEGER NOT NULL DEFAULT 1,   -- privacy flag, drives export redaction
    notes       TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE names (                           -- alternate names: maiden, married, aka...
    id            INTEGER PRIMARY KEY,
    individual_id INTEGER NOT NULL REFERENCES individuals(id) ON DELETE CASCADE,
    kind          TEXT NOT NULL,               -- 'birth','married','aka','religious'
    given_name    TEXT,
    surname       TEXT,
    name_prefix   TEXT,
    name_suffix   TEXT,
    sort_order    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE families (                        -- a union of (up to) two partners
    id          INTEGER PRIMARY KEY,
    partner1_id INTEGER REFERENCES individuals(id) ON DELETE SET NULL,
    partner2_id INTEGER REFERENCES individuals(id) ON DELETE SET NULL,
    union_type  TEXT,                          -- 'marriage','partnership','unknown'
    notes       TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE family_children (
    family_id  INTEGER NOT NULL REFERENCES families(id)    ON DELETE CASCADE,
    child_id   INTEGER NOT NULL REFERENCES individuals(id) ON DELETE CASCADE,
    relation   TEXT NOT NULL DEFAULT 'birth',  -- 'birth','adopted','step','foster'
    sort_order INTEGER NOT NULL DEFAULT 0,     -- birth order
    PRIMARY KEY (family_id, child_id)
);

CREATE TABLE places (
    id        INTEGER PRIMARY KEY,
    name      TEXT NOT NULL,                   -- free-form full place string
    latitude  REAL,
    longitude REAL,
    parent_id INTEGER REFERENCES places(id) ON DELETE SET NULL
);

CREATE TABLE events (                          -- belongs to an individual XOR a family
    id            INTEGER PRIMARY KEY,
    individual_id INTEGER REFERENCES individuals(id) ON DELETE CASCADE,
    family_id     INTEGER REFERENCES families(id)    ON DELETE CASCADE,
    kind          TEXT NOT NULL,               -- 'birth','death','marriage','divorce',...
    date_original TEXT,                        -- raw string as entered/imported
    date_modifier TEXT,                        -- 'exact','about','before','after',
                                               --  'between','estimated','calculated'
    date_sort     INTEGER,                     -- sortable key: proleptic day of best estimate
    date_year     INTEGER,                     -- parsed best-estimate components (nullable)
    date_month    INTEGER,
    date_day      INTEGER,
    place_id      INTEGER REFERENCES places(id) ON DELETE SET NULL,
    notes         TEXT,
    CHECK ((individual_id IS NULL) <> (family_id IS NULL))   -- exactly one subject
);

CREATE TABLE sources (
    id          INTEGER PRIMARY KEY,
    title       TEXT NOT NULL,
    author      TEXT,
    publication TEXT,
    repository  TEXT,
    notes       TEXT
);

CREATE TABLE citations (                        -- links a source to a fact
    id            INTEGER PRIMARY KEY,
    source_id     INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    event_id      INTEGER REFERENCES events(id)       ON DELETE CASCADE,
    individual_id INTEGER REFERENCES individuals(id)  ON DELETE CASCADE,
    family_id     INTEGER REFERENCES families(id)     ON DELETE CASCADE,
    page          TEXT,
    detail        TEXT,
    confidence    TEXT                          -- 'primary','secondary','questionable'
);

CREATE TABLE media (
    id      INTEGER PRIMARY KEY,
    path    TEXT NOT NULL,                       -- relative path within the media folder
    caption TEXT,
    mime    TEXT
);

CREATE TABLE media_links (
    media_id      INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
    individual_id INTEGER REFERENCES individuals(id) ON DELETE CASCADE,
    family_id     INTEGER REFERENCES families(id)    ON DELETE CASCADE,
    event_id      INTEGER REFERENCES events(id)      ON DELETE CASCADE,
    is_primary    INTEGER NOT NULL DEFAULT 0
);

-- Indexes
CREATE INDEX idx_individuals_surname ON individuals(surname, given_name);
CREATE INDEX idx_events_individual   ON events(individual_id);
CREATE INDEX idx_events_family       ON events(family_id);
CREATE INDEX idx_famchildren_child   ON family_children(child_id);
CREATE INDEX idx_families_partner1   ON families(partner1_id);
CREATE INDEX idx_families_partner2   ON families(partner2_id);
