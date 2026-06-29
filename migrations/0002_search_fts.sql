-- Kith schema v2 — full-text search over individuals.
-- Append-only: never edit this file after it is applied; add a new NNNN_*.sql
-- migration instead. This is the first migration since 0001; 0001 is untouched.
--
-- An own-content FTS5 index, one row per individual (rowid = individuals.id),
-- with three columns so bm25() can weight a name hit above a note/place hit:
--   names  — core given/surname/prefix/suffix + nickname + every alternate name
--   notes  — the individual's free-form notes
--   places — the place names (and notes) of the individual's OWN events
-- A family event (individual_id IS NULL) contributes to no person's row.
--
-- The tokenizer folds diacritics, so "Bjorn" finds "Bjørn" and "Muller" finds
-- "Müller" — essential for genealogical names.

CREATE VIRTUAL TABLE person_search USING fts5(
    names,
    notes,
    places,
    tokenize = 'unicode61 remove_diacritics 2'
);

-- A person's index row is always *recomputed from the base tables* (never
-- patched incrementally), so it converges to the right document regardless of
-- the order the contributor rows were written in — which is what lets the bulk
-- GEDCOM importer (person, then names, then events, then places, in passes)
-- stay completely unchanged. Each recompute is:
--
--   INSERT OR REPLACE INTO person_search(rowid, names, notes, places)
--   SELECT i.id, <names>, <notes>, <places>
--   FROM individuals i WHERE i.id = <affected id>;
--
-- The `FROM individuals i WHERE i.id = ...` join is also the no-resurrection
-- guard: when a person is cascade-deleted, the names/events delete triggers find
-- no surviving individual row, so the SELECT is empty and nothing is written;
-- the `AFTER DELETE ON individuals` trigger purges the row by rowid. The result
-- is correct under any FK-cascade/trigger ordering (no reliance on
-- recursive_triggers).

------------------------------------------------------------------------------
-- individuals: the row's own names/notes change here.
------------------------------------------------------------------------------
CREATE TRIGGER person_search_individuals_ai AFTER INSERT ON individuals BEGIN
    INSERT OR REPLACE INTO person_search(rowid, names, notes, places)
    SELECT i.id,
        trim(coalesce(i.given_name,'')||' '||coalesce(i.surname,'')||' '||
             coalesce(i.name_prefix,'')||' '||coalesce(i.name_suffix,'')||' '||
             coalesce(i.nickname,'')||' '||
             coalesce((SELECT group_concat(trim(coalesce(n.given_name,'')||' '||
                 coalesce(n.surname,'')||' '||coalesce(n.name_prefix,'')||' '||
                 coalesce(n.name_suffix,'')),' ')
                 FROM names n WHERE n.individual_id = i.id),'')),
        coalesce(i.notes,''),
        coalesce((SELECT group_concat(trim(coalesce(p.name,'')||' '||
            coalesce(e.notes,'')),' ')
            FROM events e LEFT JOIN places p ON p.id = e.place_id
            WHERE e.individual_id = i.id),'')
    FROM individuals i WHERE i.id = new.id;
END;

CREATE TRIGGER person_search_individuals_au AFTER UPDATE ON individuals BEGIN
    INSERT OR REPLACE INTO person_search(rowid, names, notes, places)
    SELECT i.id,
        trim(coalesce(i.given_name,'')||' '||coalesce(i.surname,'')||' '||
             coalesce(i.name_prefix,'')||' '||coalesce(i.name_suffix,'')||' '||
             coalesce(i.nickname,'')||' '||
             coalesce((SELECT group_concat(trim(coalesce(n.given_name,'')||' '||
                 coalesce(n.surname,'')||' '||coalesce(n.name_prefix,'')||' '||
                 coalesce(n.name_suffix,'')),' ')
                 FROM names n WHERE n.individual_id = i.id),'')),
        coalesce(i.notes,''),
        coalesce((SELECT group_concat(trim(coalesce(p.name,'')||' '||
            coalesce(e.notes,'')),' ')
            FROM events e LEFT JOIN places p ON p.id = e.place_id
            WHERE e.individual_id = i.id),'')
    FROM individuals i WHERE i.id = new.id;
END;

CREATE TRIGGER person_search_individuals_ad AFTER DELETE ON individuals BEGIN
    DELETE FROM person_search WHERE rowid = old.id;
END;

------------------------------------------------------------------------------
-- names: an individual's alternate names feed the `names` column.
------------------------------------------------------------------------------
CREATE TRIGGER person_search_names_ai AFTER INSERT ON names BEGIN
    INSERT OR REPLACE INTO person_search(rowid, names, notes, places)
    SELECT i.id,
        trim(coalesce(i.given_name,'')||' '||coalesce(i.surname,'')||' '||
             coalesce(i.name_prefix,'')||' '||coalesce(i.name_suffix,'')||' '||
             coalesce(i.nickname,'')||' '||
             coalesce((SELECT group_concat(trim(coalesce(n.given_name,'')||' '||
                 coalesce(n.surname,'')||' '||coalesce(n.name_prefix,'')||' '||
                 coalesce(n.name_suffix,'')),' ')
                 FROM names n WHERE n.individual_id = i.id),'')),
        coalesce(i.notes,''),
        coalesce((SELECT group_concat(trim(coalesce(p.name,'')||' '||
            coalesce(e.notes,'')),' ')
            FROM events e LEFT JOIN places p ON p.id = e.place_id
            WHERE e.individual_id = i.id),'')
    FROM individuals i WHERE i.id = new.individual_id;
END;

CREATE TRIGGER person_search_names_au AFTER UPDATE ON names BEGIN
    INSERT OR REPLACE INTO person_search(rowid, names, notes, places)
    SELECT i.id,
        trim(coalesce(i.given_name,'')||' '||coalesce(i.surname,'')||' '||
             coalesce(i.name_prefix,'')||' '||coalesce(i.name_suffix,'')||' '||
             coalesce(i.nickname,'')||' '||
             coalesce((SELECT group_concat(trim(coalesce(n.given_name,'')||' '||
                 coalesce(n.surname,'')||' '||coalesce(n.name_prefix,'')||' '||
                 coalesce(n.name_suffix,'')),' ')
                 FROM names n WHERE n.individual_id = i.id),'')),
        coalesce(i.notes,''),
        coalesce((SELECT group_concat(trim(coalesce(p.name,'')||' '||
            coalesce(e.notes,'')),' ')
            FROM events e LEFT JOIN places p ON p.id = e.place_id
            WHERE e.individual_id = i.id),'')
    FROM individuals i WHERE i.id = new.individual_id;
END;

CREATE TRIGGER person_search_names_ad AFTER DELETE ON names BEGIN
    INSERT OR REPLACE INTO person_search(rowid, names, notes, places)
    SELECT i.id,
        trim(coalesce(i.given_name,'')||' '||coalesce(i.surname,'')||' '||
             coalesce(i.name_prefix,'')||' '||coalesce(i.name_suffix,'')||' '||
             coalesce(i.nickname,'')||' '||
             coalesce((SELECT group_concat(trim(coalesce(n.given_name,'')||' '||
                 coalesce(n.surname,'')||' '||coalesce(n.name_prefix,'')||' '||
                 coalesce(n.name_suffix,'')),' ')
                 FROM names n WHERE n.individual_id = i.id),'')),
        coalesce(i.notes,''),
        coalesce((SELECT group_concat(trim(coalesce(p.name,'')||' '||
            coalesce(e.notes,'')),' ')
            FROM events e LEFT JOIN places p ON p.id = e.place_id
            WHERE e.individual_id = i.id),'')
    FROM individuals i WHERE i.id = old.individual_id;
END;

------------------------------------------------------------------------------
-- events: only an INDIVIDUAL's events feed the `places` column (family events
-- have individual_id NULL — the WHEN guard skips them).
------------------------------------------------------------------------------
CREATE TRIGGER person_search_events_ai AFTER INSERT ON events
WHEN new.individual_id IS NOT NULL BEGIN
    INSERT OR REPLACE INTO person_search(rowid, names, notes, places)
    SELECT i.id,
        trim(coalesce(i.given_name,'')||' '||coalesce(i.surname,'')||' '||
             coalesce(i.name_prefix,'')||' '||coalesce(i.name_suffix,'')||' '||
             coalesce(i.nickname,'')||' '||
             coalesce((SELECT group_concat(trim(coalesce(n.given_name,'')||' '||
                 coalesce(n.surname,'')||' '||coalesce(n.name_prefix,'')||' '||
                 coalesce(n.name_suffix,'')),' ')
                 FROM names n WHERE n.individual_id = i.id),'')),
        coalesce(i.notes,''),
        coalesce((SELECT group_concat(trim(coalesce(p.name,'')||' '||
            coalesce(e.notes,'')),' ')
            FROM events e LEFT JOIN places p ON p.id = e.place_id
            WHERE e.individual_id = i.id),'')
    FROM individuals i WHERE i.id = new.individual_id;
END;

CREATE TRIGGER person_search_events_au AFTER UPDATE ON events
WHEN new.individual_id IS NOT NULL BEGIN
    INSERT OR REPLACE INTO person_search(rowid, names, notes, places)
    SELECT i.id,
        trim(coalesce(i.given_name,'')||' '||coalesce(i.surname,'')||' '||
             coalesce(i.name_prefix,'')||' '||coalesce(i.name_suffix,'')||' '||
             coalesce(i.nickname,'')||' '||
             coalesce((SELECT group_concat(trim(coalesce(n.given_name,'')||' '||
                 coalesce(n.surname,'')||' '||coalesce(n.name_prefix,'')||' '||
                 coalesce(n.name_suffix,'')),' ')
                 FROM names n WHERE n.individual_id = i.id),'')),
        coalesce(i.notes,''),
        coalesce((SELECT group_concat(trim(coalesce(p.name,'')||' '||
            coalesce(e.notes,'')),' ')
            FROM events e LEFT JOIN places p ON p.id = e.place_id
            WHERE e.individual_id = i.id),'')
    FROM individuals i WHERE i.id = new.individual_id;
END;

CREATE TRIGGER person_search_events_ad AFTER DELETE ON events
WHEN old.individual_id IS NOT NULL BEGIN
    INSERT OR REPLACE INTO person_search(rowid, names, notes, places)
    SELECT i.id,
        trim(coalesce(i.given_name,'')||' '||coalesce(i.surname,'')||' '||
             coalesce(i.name_prefix,'')||' '||coalesce(i.name_suffix,'')||' '||
             coalesce(i.nickname,'')||' '||
             coalesce((SELECT group_concat(trim(coalesce(n.given_name,'')||' '||
                 coalesce(n.surname,'')||' '||coalesce(n.name_prefix,'')||' '||
                 coalesce(n.name_suffix,'')),' ')
                 FROM names n WHERE n.individual_id = i.id),'')),
        coalesce(i.notes,''),
        coalesce((SELECT group_concat(trim(coalesce(p.name,'')||' '||
            coalesce(e.notes,'')),' ')
            FROM events e LEFT JOIN places p ON p.id = e.place_id
            WHERE e.individual_id = i.id),'')
    FROM individuals i WHERE i.id = old.individual_id;
END;

------------------------------------------------------------------------------
-- places: a rename fans out to every person with an event there (the one
-- trigger with real reach). A new place has no events yet, so no INSERT trigger
-- is needed; the event trigger indexes the place when the event is attached.
------------------------------------------------------------------------------
CREATE TRIGGER person_search_places_au AFTER UPDATE ON places BEGIN
    INSERT OR REPLACE INTO person_search(rowid, names, notes, places)
    SELECT i.id,
        trim(coalesce(i.given_name,'')||' '||coalesce(i.surname,'')||' '||
             coalesce(i.name_prefix,'')||' '||coalesce(i.name_suffix,'')||' '||
             coalesce(i.nickname,'')||' '||
             coalesce((SELECT group_concat(trim(coalesce(n.given_name,'')||' '||
                 coalesce(n.surname,'')||' '||coalesce(n.name_prefix,'')||' '||
                 coalesce(n.name_suffix,'')),' ')
                 FROM names n WHERE n.individual_id = i.id),'')),
        coalesce(i.notes,''),
        coalesce((SELECT group_concat(trim(coalesce(p.name,'')||' '||
            coalesce(e.notes,'')),' ')
            FROM events e LEFT JOIN places p ON p.id = e.place_id
            WHERE e.individual_id = i.id),'')
    FROM individuals i
    WHERE i.id IN (SELECT DISTINCT e.individual_id FROM events e
                   WHERE e.place_id = new.id AND e.individual_id IS NOT NULL);
END;

------------------------------------------------------------------------------
-- Backfill existing rows (a no-op on a fresh database; populates the index when
-- the migration runs on an already-populated one).
------------------------------------------------------------------------------
INSERT INTO person_search(rowid, names, notes, places)
SELECT i.id,
    trim(coalesce(i.given_name,'')||' '||coalesce(i.surname,'')||' '||
         coalesce(i.name_prefix,'')||' '||coalesce(i.name_suffix,'')||' '||
         coalesce(i.nickname,'')||' '||
         coalesce((SELECT group_concat(trim(coalesce(n.given_name,'')||' '||
             coalesce(n.surname,'')||' '||coalesce(n.name_prefix,'')||' '||
             coalesce(n.name_suffix,'')),' ')
             FROM names n WHERE n.individual_id = i.id),'')),
    coalesce(i.notes,''),
    coalesce((SELECT group_concat(trim(coalesce(p.name,'')||' '||
        coalesce(e.notes,'')),' ')
        FROM events e LEFT JOIN places p ON p.id = e.place_id
        WHERE e.individual_id = i.id),'')
FROM individuals i;
