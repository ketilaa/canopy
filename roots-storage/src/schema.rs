pub const CREATE_TABLES_V3: &str = "
CREATE TABLE IF NOT EXISTS workspaces (
    id   TEXT PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS projects (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    path         TEXT NOT NULL,
    language     TEXT NOT NULL,
    UNIQUE(workspace_id, name)
);

CREATE TABLE IF NOT EXISTS files (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id TEXT NOT NULL,
    project_id   INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    path         TEXT NOT NULL,
    language     TEXT NOT NULL,
    indexed_at   TEXT NOT NULL,
    UNIQUE(workspace_id, project_id, path)
);

CREATE TABLE IF NOT EXISTS symbols (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id TEXT NOT NULL,
    project_id   INTEGER NOT NULL,
    file_id      INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    kind         TEXT NOT NULL,
    line         INTEGER NOT NULL,
    fqn          TEXT NOT NULL DEFAULT '',
    UNIQUE(workspace_id, fqn)
);

CREATE INDEX IF NOT EXISTS idx_symbols_name       ON symbols(name);
CREATE INDEX IF NOT EXISTS idx_symbols_name_lower ON symbols(LOWER(name));
CREATE INDEX IF NOT EXISTS idx_symbols_ws_name    ON symbols(workspace_id, name);

CREATE TABLE IF NOT EXISTS relationships (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id TEXT NOT NULL,
    from_symbol  TEXT NOT NULL,
    to_symbol    TEXT NOT NULL,
    kind         TEXT NOT NULL,
    file         TEXT NOT NULL,
    line         INTEGER
);

CREATE INDEX IF NOT EXISTS idx_rel_from    ON relationships(from_symbol);
CREATE INDEX IF NOT EXISTS idx_rel_to      ON relationships(to_symbol);
CREATE INDEX IF NOT EXISTS idx_rel_ws_from ON relationships(workspace_id, from_symbol);
CREATE INDEX IF NOT EXISTS idx_rel_ws_to   ON relationships(workspace_id, to_symbol);
";

// Best-effort migration for databases created before V3.
// SQLite cannot add composite UNIQUE constraints via ALTER TABLE;
// new constraints apply only to rows inserted after this migration.
// Existing V2 data is preserved under the 'default' workspace.
pub const MIGRATE_V2_TO_V3: &str = "
CREATE TABLE IF NOT EXISTS workspaces (
    id   TEXT PRIMARY KEY,
    name TEXT NOT NULL
);
INSERT OR IGNORE INTO workspaces (id, name) VALUES ('default', 'default');
ALTER TABLE projects      ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE files         ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE symbols       ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE symbols       ADD COLUMN project_id   INTEGER;
ALTER TABLE relationships ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'default';
";
