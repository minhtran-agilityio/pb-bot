-- Step 2: artifacts and their stored versions (HANDOFF §11, DESIGN_DOC "Data model").
-- An artifact with no project is in the "New artifacts" box (state 'new') or hidden (state 'ignored').
CREATE TABLE artifacts (
  id           INTEGER PRIMARY KEY,
  project_id   INTEGER REFERENCES projects(id) ON DELETE CASCADE,
  claude_id    TEXT UNIQUE,                 -- last part of the claude.ai link; NULL for a drop with no link
  link         TEXT,                        -- shown and stored only, never fetched
  name         TEXT NOT NULL,               -- slug shown on the card
  title        TEXT NOT NULL DEFAULT '',    -- log column 3, or the id
  cwd          TEXT,                        -- log column 5, used only for project auto-select
  source       TEXT NOT NULL CHECK (source IN ('hook', 'manual')),
  state        TEXT NOT NULL DEFAULT 'new' CHECK (state IN ('new', 'added', 'ignored')),
  updated      INTEGER NOT NULL DEFAULT 0,  -- 1 = a version arrived after it was added; cleared on preview
  published_at TEXT,                        -- newest publish time from the log, else the newest version time
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  UNIQUE (project_id, name),
  CHECK ((state = 'added') = (project_id IS NOT NULL))
);

CREATE TABLE artifact_versions (
  id          INTEGER PRIMARY KEY,
  artifact_id INTEGER NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
  n           INTEGER NOT NULL,             -- 1, 2, 3 per artifact
  html        TEXT NOT NULL,                -- at most 5 MiB
  sha256      TEXT NOT NULL,                -- of the stored text; a new version is skipped only when it equals the latest
  source      TEXT NOT NULL CHECK (source IN ('hook', 'drop', 'paste')),
  file        TEXT,                         -- the hook file it came from, else NULL
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  UNIQUE (artifact_id, n)
);
-- A hook file is imported once (file identity); a revert A, B, A is still three versions.
CREATE UNIQUE INDEX artifact_versions_file ON artifact_versions (file) WHERE file IS NOT NULL;

-- Hook files the folder scan already handled, so an unchanged file is not read again.
CREATE TABLE hook_files (
  path  TEXT PRIMARY KEY,
  size  INTEGER NOT NULL,
  mtime INTEGER NOT NULL
);

-- Key/value state of the log reader: log_offset and log_last_line (HANDOFF §11.4.1).
CREATE TABLE hook_state (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
