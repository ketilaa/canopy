use std::path::Path;

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use roots_core::{Language, Relationship, Symbol};

use crate::error::StorageError;
use crate::schema::{CREATE_TABLES_V3, MIGRATE_V2_TO_V3};

pub struct Store {
    conn: Connection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRow {
    pub id:   String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolRow {
    pub name:         String,
    pub kind:         String,
    pub file:         String,
    pub language:     String,
    pub project:      String,
    pub workspace_id: String,
    pub line:         u32,
    pub fqn:          String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipRow {
    pub from_symbol:  String,
    pub to_symbol:    String,
    pub kind:         String,
    pub file:         String,
    pub line:         Option<u32>,
    pub workspace_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphResult {
    pub outgoing: Vec<RelationshipRow>,
    pub incoming: Vec<RelationshipRow>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusReport {
    pub workspaces:    i64,
    pub projects:      i64,
    pub files:         i64,
    pub symbols:       i64,
    pub relationships: i64,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        Ok(Self { conn })
    }

    pub fn open_in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(Self { conn })
    }

    pub fn init_schema(&self) -> Result<(), StorageError> {
        self.conn.execute_batch(CREATE_TABLES_V3)?;
        // Best-effort migration for databases created before V3.
        let _ = self.conn.execute_batch(MIGRATE_V2_TO_V3);
        Ok(())
    }

    // --- Workspace methods ---

    pub fn upsert_workspace(&self, id: &str, name: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO workspaces (id, name) VALUES (?1, ?2)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name",
            params![id, name],
        )?;
        Ok(())
    }

    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceRow>, StorageError> {
        let mut stmt = self.conn.prepare("SELECT id, name FROM workspaces ORDER BY id")?;
        let rows = stmt.query_map([], |row| {
            Ok(WorkspaceRow {
                id:   row.get(0)?,
                name: row.get(1)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn workspace_exists(&self, id: &str) -> Result<bool, StorageError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM workspaces WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    // --- Project methods ---

    pub fn upsert_project(
        &self,
        workspace_id: &str,
        name: &str,
        path: &str,
        language: &Language,
    ) -> Result<i64, StorageError> {
        self.conn.execute(
            "INSERT INTO projects (workspace_id, name, path, language) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(workspace_id, name) DO UPDATE SET path = excluded.path, language = excluded.language",
            params![workspace_id, name, path, language.as_str()],
        )?;
        let id: i64 = self.conn.query_row(
            "SELECT id FROM projects WHERE workspace_id = ?1 AND name = ?2",
            params![workspace_id, name],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    // --- File methods ---

    pub fn upsert_file(
        &self,
        workspace_id: &str,
        project_id: i64,
        path: &str,
        language: &Language,
        indexed_at: &str,
    ) -> Result<i64, StorageError> {
        self.conn.execute(
            "INSERT INTO files (workspace_id, project_id, path, language, indexed_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(workspace_id, project_id, path) DO UPDATE SET
                 language = excluded.language, indexed_at = excluded.indexed_at",
            params![workspace_id, project_id, path, language.as_str(), indexed_at],
        )?;
        let id: i64 = self.conn.query_row(
            "SELECT id FROM files WHERE workspace_id = ?1 AND project_id = ?2 AND path = ?3",
            params![workspace_id, project_id, path],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    // --- Symbol methods ---

    pub fn delete_symbols_for_file(&self, file_id: i64) -> Result<(), StorageError> {
        self.conn.execute("DELETE FROM symbols WHERE file_id = ?1", params![file_id])?;
        Ok(())
    }

    pub fn insert_symbols(
        &self,
        workspace_id: &str,
        project_id: i64,
        file_id: i64,
        symbols: &[Symbol],
    ) -> Result<(), StorageError> {
        let mut stmt = self.conn.prepare_cached(
            "INSERT INTO symbols (workspace_id, project_id, file_id, name, kind, line, fqn)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(workspace_id, fqn) DO UPDATE SET
                 project_id = excluded.project_id,
                 file_id    = excluded.file_id,
                 name       = excluded.name,
                 kind       = excluded.kind,
                 line       = excluded.line",
        )?;
        for sym in symbols {
            stmt.execute(params![
                workspace_id,
                project_id,
                file_id,
                sym.name,
                sym.kind.as_str(),
                sym.line,
                sym.fqn,
            ])?;
        }
        Ok(())
    }

    // --- Relationship methods ---

    pub fn delete_relationships_for_file(
        &self,
        workspace_id: &str,
        file: &str,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "DELETE FROM relationships WHERE workspace_id = ?1 AND file = ?2",
            params![workspace_id, file],
        )?;
        Ok(())
    }

    pub fn insert_relationships(
        &self,
        workspace_id: &str,
        file: &str,
        rels: &[Relationship],
    ) -> Result<(), StorageError> {
        let mut stmt = self.conn.prepare_cached(
            "INSERT INTO relationships (workspace_id, from_symbol, to_symbol, kind, file, line)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for rel in rels {
            stmt.execute(params![
                workspace_id,
                rel.from_symbol,
                rel.to_symbol,
                rel.kind.as_str(),
                file,
                rel.line,
            ])?;
        }
        Ok(())
    }

    // --- Query methods (all workspace-scoped) ---

    pub fn query_by_fqn(&self, workspace_id: &str, fqn: &str) -> Result<Option<SymbolRow>, StorageError> {
        let mut results = self.run_symbol_query(
            "SELECT s.name, s.kind, f.path, f.language, p.name, s.workspace_id, s.line, s.fqn
             FROM symbols s
             JOIN files f ON f.id = s.file_id
             JOIN projects p ON p.id = f.project_id
             WHERE s.workspace_id = ?1 AND s.fqn = ?2
             LIMIT 1",
            params![workspace_id, fqn],
        )?;
        Ok(results.pop())
    }

    pub fn query_exact(&self, workspace_id: &str, name: &str) -> Result<Vec<SymbolRow>, StorageError> {
        self.run_symbol_query(
            "SELECT s.name, s.kind, f.path, f.language, p.name, s.workspace_id, s.line, s.fqn
             FROM symbols s
             JOIN files f ON f.id = s.file_id
             JOIN projects p ON p.id = f.project_id
             WHERE s.workspace_id = ?1 AND s.name = ?2
             ORDER BY p.name, f.path, s.line",
            params![workspace_id, name],
        )
    }

    pub fn query_prefix(&self, workspace_id: &str, term: &str) -> Result<Vec<SymbolRow>, StorageError> {
        let pattern = format!("%{}%", term.to_lowercase());
        self.run_symbol_query(
            "SELECT s.name, s.kind, f.path, f.language, p.name, s.workspace_id, s.line, s.fqn
             FROM symbols s
             JOIN files f ON f.id = s.file_id
             JOIN projects p ON p.id = f.project_id
             WHERE s.workspace_id = ?1 AND LOWER(s.name) LIKE ?2
             ORDER BY p.name, f.path, s.line",
            params![workspace_id, pattern],
        )
    }

    pub fn dump_all(&self, workspace_id: &str) -> Result<Vec<SymbolRow>, StorageError> {
        self.run_symbol_query(
            "SELECT s.name, s.kind, f.path, f.language, p.name, s.workspace_id, s.line, s.fqn
             FROM symbols s
             JOIN files f ON f.id = s.file_id
             JOIN projects p ON p.id = f.project_id
             WHERE s.workspace_id = ?1
             ORDER BY p.name, f.path, s.line",
            params![workspace_id],
        )
    }

    pub fn query_callers(
        &self,
        workspace_id: &str,
        fqn: &str,
    ) -> Result<Vec<RelationshipRow>, StorageError> {
        self.run_rel_query(
            "SELECT from_symbol, to_symbol, kind, file, line, workspace_id
             FROM relationships
             WHERE workspace_id = ?1 AND to_symbol = ?2 AND kind = 'CALLS'",
            params![workspace_id, fqn],
        )
    }

    pub fn query_callees(
        &self,
        workspace_id: &str,
        fqn: &str,
    ) -> Result<Vec<RelationshipRow>, StorageError> {
        self.run_rel_query(
            "SELECT from_symbol, to_symbol, kind, file, line, workspace_id
             FROM relationships
             WHERE workspace_id = ?1 AND from_symbol = ?2 AND kind = 'CALLS'",
            params![workspace_id, fqn],
        )
    }

    pub fn query_deps(
        &self,
        workspace_id: &str,
        fqn: &str,
    ) -> Result<Vec<RelationshipRow>, StorageError> {
        self.run_rel_query(
            "SELECT from_symbol, to_symbol, kind, file, line, workspace_id
             FROM relationships
             WHERE workspace_id = ?1 AND from_symbol = ?2",
            params![workspace_id, fqn],
        )
    }

    pub fn query_impact(
        &self,
        workspace_id: &str,
        fqn: &str,
    ) -> Result<Vec<String>, StorageError> {
        let mut stmt = self.conn.prepare(
            "WITH RECURSIVE impact(sym) AS (
               SELECT from_symbol FROM relationships WHERE workspace_id = ?1 AND to_symbol = ?2
               UNION
               SELECT r.from_symbol FROM relationships r
               JOIN impact i ON r.to_symbol = i.sym
               WHERE r.workspace_id = ?1
             )
             SELECT DISTINCT sym FROM impact",
        )?;
        let rows = stmt.query_map(params![workspace_id, fqn], |row| row.get(0))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn query_graph(
        &self,
        workspace_id: &str,
        fqn: &str,
    ) -> Result<GraphResult, StorageError> {
        let outgoing = self.run_rel_query(
            "SELECT from_symbol, to_symbol, kind, file, line, workspace_id
             FROM relationships WHERE workspace_id = ?1 AND from_symbol = ?2",
            params![workspace_id, fqn],
        )?;
        let incoming = self.run_rel_query(
            "SELECT from_symbol, to_symbol, kind, file, line, workspace_id
             FROM relationships WHERE workspace_id = ?1 AND to_symbol = ?2",
            params![workspace_id, fqn],
        )?;
        Ok(GraphResult { outgoing, incoming })
    }

    pub fn status(&self, workspace_id: &str) -> Result<StatusReport, StorageError> {
        let workspaces: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM workspaces", [], |r| r.get(0)
        )?;
        let projects: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM projects WHERE workspace_id = ?1",
            params![workspace_id], |r| r.get(0)
        )?;
        let files: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM files WHERE workspace_id = ?1",
            params![workspace_id], |r| r.get(0)
        )?;
        let symbols: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM symbols WHERE workspace_id = ?1",
            params![workspace_id], |r| r.get(0)
        )?;
        let relationships: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM relationships WHERE workspace_id = ?1",
            params![workspace_id], |r| r.get(0)
        )?;
        Ok(StatusReport { workspaces, projects, files, symbols, relationships })
    }

    pub fn query_project_symbols(
        &self,
        workspace_id: &str,
        project_name: &str,
    ) -> Result<Vec<SymbolRow>, StorageError> {
        self.run_symbol_query(
            "SELECT s.name, s.kind, f.path, f.language, p.name, s.workspace_id, s.line, s.fqn
             FROM symbols s
             JOIN files f ON f.id = s.file_id
             JOIN projects p ON p.id = f.project_id
             WHERE s.workspace_id = ?1 AND p.name = ?2
             ORDER BY f.path, s.line",
            params![workspace_id, project_name],
        )
    }

    pub fn query_file_symbols(
        &self,
        workspace_id: &str,
        file_path: &str,
    ) -> Result<Vec<SymbolRow>, StorageError> {
        self.run_symbol_query(
            "SELECT s.name, s.kind, f.path, f.language, p.name, s.workspace_id, s.line, s.fqn
             FROM symbols s
             JOIN files f ON f.id = s.file_id
             JOIN projects p ON p.id = f.project_id
             WHERE s.workspace_id = ?1 AND f.path = ?2
             ORDER BY s.line",
            params![workspace_id, file_path],
        )
    }

    /// Returns the stored `indexed_at` timestamp for a file, or None if not yet indexed.
    pub fn file_indexed_at(
        &self,
        workspace_id: &str,
        project_id: i64,
        path: &str,
    ) -> Result<Option<String>, StorageError> {
        let result = self.conn.query_row(
            "SELECT indexed_at FROM files \
             WHERE workspace_id = ?1 AND project_id = ?2 AND path = ?3",
            params![workspace_id, project_id, path],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Sqlite(e)),
        }
    }

    pub fn query_file_relationships(
        &self,
        workspace_id: &str,
        file_path: &str,
    ) -> Result<Vec<RelationshipRow>, StorageError> {
        self.run_rel_query(
            "SELECT from_symbol, to_symbol, kind, file, line, workspace_id
             FROM relationships
             WHERE workspace_id = ?1 AND file = ?2",
            params![workspace_id, file_path],
        )
    }

    fn run_symbol_query(
        &self,
        sql: &str,
        params: impl rusqlite::Params,
    ) -> Result<Vec<SymbolRow>, StorageError> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params, |row| {
            Ok(SymbolRow {
                name:         row.get(0)?,
                kind:         row.get(1)?,
                file:         row.get(2)?,
                language:     row.get(3)?,
                project:      row.get(4)?,
                workspace_id: row.get(5)?,
                line:         row.get(6)?,
                fqn:          row.get(7)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    fn run_rel_query(
        &self,
        sql: &str,
        params: impl rusqlite::Params,
    ) -> Result<Vec<RelationshipRow>, StorageError> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params, |row| {
            Ok(RelationshipRow {
                from_symbol:  row.get(0)?,
                to_symbol:    row.get(1)?,
                kind:         row.get(2)?,
                file:         row.get(3)?,
                line:         row.get(4)?,
                workspace_id: row.get(5)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }
}

impl Store {
    pub fn begin_transaction(&self) -> Result<(), StorageError> {
        self.conn.execute_batch("BEGIN")?;
        Ok(())
    }

    pub fn commit_transaction(&self) -> Result<(), StorageError> {
        self.conn.execute_batch("COMMIT")?;
        Ok(())
    }

    pub fn rollback_transaction(&self) -> Result<(), StorageError> {
        self.conn.execute_batch("ROLLBACK")?;
        Ok(())
    }
}
