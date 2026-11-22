-- Step 1: one row per project (= one GitHub repo and its local folder).
CREATE TABLE projects (
  id         INTEGER PRIMARY KEY,
  repo       TEXT NOT NULL UNIQUE,        -- owner/name on GitHub
  name       TEXT NOT NULL,               -- the name part, shown in the UI
  path       TEXT NOT NULL,               -- absolute path of the local checkout
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
