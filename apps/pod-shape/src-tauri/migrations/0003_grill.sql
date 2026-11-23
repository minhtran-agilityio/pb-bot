-- Step 3: grill sessions, their history, briefs, features and every `claude -p` run
-- (DESIGN_DOC "Data model"; specs design §6, advisor-reviewed).
-- Change from DESIGN_DOC: a brief hangs on its grill session, because it exists before the owner
-- confirms it. The feature (1 grill = 1 feature) is made by the confirmation, so the brief's feature
-- is derived through the session; there is no briefs.feature_id or features.artifact_id to drift.

CREATE TABLE grill_sessions (
  id                  INTEGER PRIMARY KEY,
  artifact_version_id INTEGER NOT NULL REFERENCES artifact_versions(id) ON DELETE RESTRICT,
  claude_session_id   TEXT NOT NULL UNIQUE,        -- uuid for --session-id, then --resume
  n                   INTEGER NOT NULL,            -- "Session 2", per artifact
  artifact_id         INTEGER NOT NULL REFERENCES artifacts(id) ON DELETE RESTRICT,
  state               TEXT NOT NULL CHECK (state IN
                        ('running', 'waiting_owner', 'brief_ready', 'confirmed', 'stopped', 'failed')),
  claude_started      INTEGER NOT NULL DEFAULT 0,  -- an init event arrived: later turns use --resume
  turns               INTEGER NOT NULL DEFAULT 0,
  pending_message     TEXT,                        -- the message of the turn that did not finish
  draft_answers       TEXT,                        -- answers given so far to the open block (JSON object)
  adr_pending         INTEGER NOT NULL DEFAULT 0,  -- the owner confirmed an ADR; the app runs git after the turn
  app_note            TEXT,                        -- told to Claude at the start of the next message (ADR result)
  error               TEXT,                        -- the last real error, shown as is
  created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  ended_at            TEXT,
  UNIQUE (artifact_id, n)
);

CREATE TABLE grill_messages (
  id          INTEGER PRIMARY KEY,
  session_id  INTEGER NOT NULL REFERENCES grill_sessions(id) ON DELETE CASCADE,
  turn        INTEGER NOT NULL,
  role        TEXT NOT NULL CHECK (role IN ('owner', 'claude', 'tool', 'error', 'note')),
  text        TEXT NOT NULL,                       -- shown text; a p2g block is cut out of claude text
  questions   TEXT,                                -- the validated p2g-questions JSON (claude rows)
  answered    INTEGER NOT NULL DEFAULT 0,
  interrupted INTEGER NOT NULL DEFAULT 0,          -- the turn was stopped or the app closed
  raw         TEXT,                                -- the sent message, or the used part of the event
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX grill_messages_session ON grill_messages (session_id, id);

CREATE TABLE briefs (
  id               INTEGER PRIMARY KEY,
  grill_session_id INTEGER NOT NULL REFERENCES grill_sessions(id) ON DELETE CASCADE,
  n                INTEGER NOT NULL,
  path             TEXT NOT NULL,                  -- specs/<slug>/brief.md, relative to the project folder
  content          TEXT NOT NULL,
  sha256           TEXT NOT NULL,
  confirmed_at     TEXT,
  created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  UNIQUE (grill_session_id, n)
);

CREATE TABLE features (
  id               INTEGER PRIMARY KEY,
  grill_session_id INTEGER NOT NULL UNIQUE REFERENCES grill_sessions(id) ON DELETE RESTRICT,
  name             TEXT NOT NULL,
  slug             TEXT NOT NULL,
  state            TEXT NOT NULL DEFAULT 'confirmed' CHECK (state IN ('confirmed', 'planned', 'issued')),
  created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE runs (
  id               INTEGER PRIMARY KEY,
  grill_session_id INTEGER REFERENCES grill_sessions(id) ON DELETE CASCADE,
  feature_id       INTEGER REFERENCES features(id) ON DELETE CASCADE,
  step             TEXT NOT NULL CHECK (step IN ('grill', 'adr', 'tickets', 'create')),
  command          TEXT NOT NULL,                  -- one line for the RunLog
  args             TEXT NOT NULL,                  -- full argv as JSON
  pgid             INTEGER,                        -- process group, for the orphan sweep at start
  proc_start       TEXT,                           -- `ps -o lstart=` of that pid, so a reused pid is not killed
  exit_code        INTEGER,
  error            TEXT,
  cost_usd         REAL,                           -- cumulative over --resume (spike RESULTS)
  started_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  ended_at         TEXT,
  ms               INTEGER,
  CHECK (grill_session_id IS NOT NULL OR feature_id IS NOT NULL)
);
CREATE INDEX runs_session ON runs (grill_session_id, id);
CREATE INDEX runs_live ON runs (pgid) WHERE ended_at IS NULL;
