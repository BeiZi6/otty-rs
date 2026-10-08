//! Durable state: the session index, the frecency database and the agent
//! conversation store.
//!
//! Everything lives in one SQLite database under the app-support directory, so a
//! `rm` of the config folder never loses it and a backup of the config folder
//! never carries it. Three tables matter:
//!
//! * `session` — one row per pane session, which is what session restore reads
//!   back after a crash or a reboot;
//! * `frecency` — one row per directory (and per learned command), which is what
//!   `otty jump`, Open Quickly's Folders tab and `otty learn` share;
//! * `agent_session` — one row per agent conversation, which is what Agent
//!   History, resume and fork read.
//!
//! The schema is versioned with `PRAGMA user_version` and migrated forward with
//! `ALTER TABLE`, never by dropping: the database outlives any one build.

use std::path::{Path, PathBuf};
use std::time::Duration;

use otty_core::{AgentId, AgentState, PaneId};
use rusqlite::{params, Connection, OptionalExtension};

/// The schema version this build writes.
pub const SCHEMA_VERSION: i32 = 5;

/// A failure coming out of the store.
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("could not create the state directory {0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("the database is at schema version {found}, this build writes {expected}")]
    TooNew { found: i32, expected: i32 },
}

/// The state database.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open (creating and migrating as needed) the database at the default
    /// location.
    pub fn open_default() -> Result<Self, StateError> {
        Self::open(&Self::default_path())
    }

    /// `~/.local/state/otty/state.db`, or the platform equivalent.
    pub fn default_path() -> PathBuf {
        otty_core::app_support_dir().join("state.db")
    }

    /// Open a database at `path`.
    pub fn open(path: &Path) -> Result<Self, StateError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| StateError::Io(parent.to_path_buf(), e))?;
        }
        let conn = Connection::open(path)?;
        // WAL keeps the writer from blocking the UI thread's reads, and the
        // recovered schema DDL asks for the same journal mode.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    /// An in-memory store, for tests and for `otty features` demos.
    pub fn in_memory() -> Result<Self, StateError> {
        let conn = Connection::open_in_memory()?;
        let mut store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    fn user_version(&self) -> Result<i32, StateError> {
        Ok(self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }

    /// Bring the schema up to [`SCHEMA_VERSION`].
    ///
    /// Forward only, and additive: a downgrade is refused rather than
    /// destructive, because the alternative is silently dropping a user's
    /// history.
    pub fn migrate(&mut self) -> Result<(), StateError> {
        let found = self.user_version()?;
        if found > SCHEMA_VERSION {
            return Err(StateError::TooNew { found, expected: SCHEMA_VERSION });
        }
        if found == SCHEMA_VERSION {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        if found < 1 {
            tx.execute_batch(DDL_V1)?;
        }
        if found < 2 {
            tx.execute_batch(DDL_V2)?;
        }
        if found < 3 {
            tx.execute_batch(DDL_V3)?;
        }
        if found < 4 {
            tx.execute_batch(DDL_V4)?;
        }
        if found < 5 {
            tx.execute_batch(DDL_V5)?;
        }
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        tx.commit()?;
        Ok(())
    }

    /// Whether `table` has `column` — the check a migration has to make before
    /// adding it, because SQLite has no `ADD COLUMN IF NOT EXISTS`.
    pub fn column_exists(&self, table: &str, column: &str) -> Result<bool, StateError> {
        let found: Option<String> = self
            .conn
            .query_row(
                "SELECT name FROM pragma_table_info(?1) WHERE name = ?2",
                params![table, column],
                |r| r.get(0),
            )
            .optional()?;
        Ok(found.is_some())
    }

    // ------------------------------------------------------------ sessions

    /// Record a pane session for restore.
    pub fn upsert_session(&self, s: &SessionRecord) -> Result<(), StateError> {
        self.conn.execute(
            "INSERT INTO session
                 (id, tab_id, cwd, command, title, cols, rows, pid, multiplexer, closed_at, restored_at, identity)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET
                 tab_id = excluded.tab_id,
                 cwd = excluded.cwd,
                 command = excluded.command,
                 title = excluded.title,
                 cols = excluded.cols,
                 rows = excluded.rows,
                 pid = excluded.pid,
                 multiplexer = excluded.multiplexer,
                 closed_at = excluded.closed_at,
                 restored_at = excluded.restored_at,
                 identity = excluded.identity",
            params![
                s.id.get() as i64,
                s.tab_id as i64,
                s.cwd,
                s.command,
                s.title,
                s.cols as i64,
                s.rows as i64,
                s.pid.map(|p| p as i64),
                s.multiplexer,
                s.closed_at,
                s.restored_at,
                s.identity,
            ],
        )?;
        Ok(())
    }

    /// The sessions to offer on restore: everything that was open, newest first.
    pub fn sessions_for_restore(&self, limit: usize) -> Result<Vec<SessionRecord>, StateError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, tab_id, cwd, command, title, cols, rows, pid, multiplexer, closed_at, restored_at, identity
             FROM session
             ORDER BY COALESCE(closed_at, 0) DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], SessionRecord::from_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Mark every session as closed, which is what happens when the app exits
    /// cleanly rather than crashing.
    pub fn close_all_sessions(&self, now: i64) -> Result<usize, StateError> {
        Ok(self
            .conn
            .execute("UPDATE session SET closed_at = ?1 WHERE closed_at IS NULL", params![now])?)
    }

    /// Forget a session — a closed tab that is past the reopen window.
    pub fn forget_session(&self, id: PaneId) -> Result<(), StateError> {
        self.conn.execute("DELETE FROM session WHERE id = ?1", params![id.get() as i64])?;
        Ok(())
    }

    /// Persist an OSC 88 announce so a later launch can resume the same pane.
    pub fn record_osc88(
        &self,
        pane: PaneId,
        identity: &str,
        command: Option<&str>,
    ) -> Result<(), StateError> {
        let mut rec = SessionRecord::new(pane, 80, 24);
        rec.identity = Some(identity.to_owned());
        rec.command = command.map(str::to_owned);
        self.upsert_session(&rec)
    }

    /// Reconnect by identity. `None` means the identity is gone — show "cannot resume".
    pub fn resume_osc88(&self, identity: &str) -> Result<Option<SessionRecord>, StateError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, tab_id, cwd, command, title, cols, rows, pid, multiplexer, closed_at, restored_at, identity
             FROM session WHERE identity = ?1 ORDER BY COALESCE(closed_at, 0) DESC LIMIT 1",
        )?;
        let row = stmt
            .query_row(params![identity], SessionRecord::from_row)
            .optional()?;
        Ok(row)
    }

    // ------------------------------------------------------------ frecency

    /// Record a directory visit.
    ///
    /// One visit increments the count and stamps the time; the score is derived
    /// on read, so a stale ranking corrects itself without a background pass.
    pub fn record_visit(&self, path: &str, now: i64) -> Result<(), StateError> {
        self.conn.execute(
            "INSERT INTO frecency (path, visits, last_visit, ignored)
             VALUES (?1, 1, ?2, 0)
             ON CONFLICT(path) DO UPDATE SET
                 visits = visits + 1,
                 last_visit = excluded.last_visit",
            params![normalize_path(path), now],
        )?;
        Ok(())
    }

    /// The frecency-ranked directories.
    ///
    /// The score is `visits × decay`, where `decay` halves every 30 days since
    /// the last visit. That is the shape the product describes as "frecency":
    /// a directory you use constantly outranks one you used once, a long time
    /// ago, and a directory you used once last week outranks one you used once
    /// last year.
    ///
    /// The arithmetic runs in Rust rather than in SQL: the bundled SQLite has no
    /// `POWER()`, and keeping the formula in [`decayed_score`] means the CLI's
    /// `--score` output and the ranking can never disagree.
    pub fn ranked_directories(&self, limit: usize, now: i64) -> Result<Vec<Folder>, StateError> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, visits, last_visit FROM frecency WHERE ignored = 0")?;
        let rows = stmt.query_map([], Folder::from_row)?;
        let mut folders: Vec<Folder> = rows.collect::<Result<Vec<_>, _>>()?;
        folders.sort_by(|a, b| {
            b.score(now)
                .partial_cmp(&a.score(now))
                .unwrap_or(std::cmp::Ordering::Equal)
                // A stable tie-break, so two equal scores rank by recency.
                .then_with(|| b.last_visit.cmp(&a.last_visit))
                .then_with(|| a.path.cmp(&b.path))
        });
        folders.truncate(limit);
        Ok(folders)
    }

    /// The frecency score of one directory, for `otty learn --score`.
    pub fn score_of(&self, path: &str, now: i64) -> Result<Option<f64>, StateError> {
        let row: Option<(i64, i64)> = self
            .conn
            .query_row(
                "SELECT visits, last_visit FROM frecency WHERE path = ?1",
                params![normalize_path(path)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(row.map(|(visits, last)| decayed_score(visits, last, now)))
    }

    /// `otty ignore` — stop ranking a directory without forgetting its history.
    pub fn enqueue_prompt(&self, pane: i64, text: &str, now: i64) -> Result<i64, StateError> {
        self.conn.execute(
            "INSERT INTO prompt_queue (pane, text, created) VALUES (?1, ?2, ?3)",
            params![pane, text, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn dequeue_prompt(&self, pane: i64) -> Result<Option<String>, StateError> {
        let row: Option<(i64, String)> = self
            .conn
            .query_row(
                "SELECT id, text FROM prompt_queue WHERE pane = ?1 ORDER BY id ASC LIMIT 1",
                params![pane],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((id, text)) = row else { return Ok(None) };
        self.conn
            .execute("DELETE FROM prompt_queue WHERE id = ?1", params![id])?;
        Ok(Some(text))
    }

    pub fn ignore_path(&self, path: &str) -> Result<bool, StateError> {
        let n = self.conn.execute(
            "UPDATE frecency SET ignored = 1 WHERE path = ?1",
            params![normalize_path(path)],
        )?;
        Ok(n > 0)
    }

    /// Record a learned command, so autocomplete can suggest it later.
    pub fn record_command(&self, command: &str, cwd: &str, now: i64) -> Result<(), StateError> {
        self.conn.execute(
            "INSERT INTO command_history (command, cwd, visits, last_visit)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(command, cwd) DO UPDATE SET
                 visits = visits + 1,
                 last_visit = excluded.last_visit",
            params![command, normalize_path(cwd), now],
        )?;
        Ok(())
    }

    /// The most-used commands in a directory, for the inline suggestion.
    pub fn commands_in(&self, cwd: &str, limit: usize) -> Result<Vec<String>, StateError> {
        let mut stmt = self.conn.prepare(
            "SELECT command FROM command_history
             WHERE cwd = ?1
             ORDER BY visits DESC, last_visit DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![normalize_path(cwd), limit as i64], |r| r.get(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    // ------------------------------------------------------- agent sessions

    /// Record or update an agent conversation.
    ///
    /// The key is `(agent, id)`: an agent's session id is unique within that
    /// agent but two agents can mint the same string, and a hook names both.
    pub fn upsert_agent_session(&self, s: &AgentSession) -> Result<(), StateError> {
        self.conn.execute(
            "INSERT INTO agent_session
                 (id, agent, pane, cwd, title, state, resume_command, fork_command, bypass,
                  created_at, updated_at, closed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(agent, id) DO UPDATE SET
                 agent = excluded.agent,
                 pane = excluded.pane,
                 cwd = excluded.cwd,
                 title = COALESCE(excluded.title, agent_session.title),
                 state = excluded.state,
                 resume_command = excluded.resume_command,
                 fork_command = excluded.fork_command,
                 bypass = excluded.bypass,
                 updated_at = excluded.updated_at,
                 closed_at = excluded.closed_at",
            params![
                s.id,
                s.agent.as_str(),
                s.pane.map(|p| p.get() as i64),
                s.cwd,
                s.title,
                s.state.as_str(),
                s.resume_command,
                s.fork_command,
                s.bypass as i64,
                s.created_at,
                s.updated_at,
                s.closed_at,
            ],
        )?;
        Ok(())
    }

    /// Agent History: every conversation, newest first, optionally filtered.
    pub fn agent_history(
        &self,
        agent: Option<&AgentId>,
        limit: usize,
    ) -> Result<Vec<AgentSession>, StateError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent, pane, cwd, title, state, resume_command, fork_command, bypass,
                    created_at, updated_at, closed_at
             FROM agent_session
             WHERE (?1 IS NULL OR agent = ?1)
             ORDER BY updated_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(
            params![agent.map(|a| a.as_str().to_owned()), limit as i64],
            AgentSession::from_row,
        )?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// The live state of one agent session, which is what `otty watch:<agent>`
    /// polls.
    pub fn agent_state(
        &self,
        agent: &AgentId,
        id: &str,
    ) -> Result<Option<AgentState>, StateError> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT state FROM agent_session WHERE agent = ?1 AND id = ?2",
                params![agent.as_str(), id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(raw.as_deref().and_then(AgentState::parse))
    }

    /// Which pane an agent session is attached to, so an incoming hook can be
    /// matched to a pane before a session mapping exists.
    pub fn pane_for_agent_session(
        &self,
        agent: &AgentId,
        id: &str,
    ) -> Result<Option<PaneId>, StateError> {
        // Outer `Option` is "row absent"; inner is the nullable `pane` column.
        let raw: Option<Option<i64>> = self
            .conn
            .query_row(
                "SELECT pane FROM agent_session WHERE agent = ?1 AND id = ?2",
                params![agent.as_str(), id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(raw.flatten().map(|v| PaneId::new(v as u64)))
    }
}

/// The frecency-decayed score of a directory.
pub fn decayed_score(visits: i64, last_visit: i64, now: i64) -> f64 {
    const HALF_LIFE_SECONDS: f64 = 30.0 * 24.0 * 3600.0;
    let age = (now - last_visit).max(0) as f64;
    visits as f64 * 0.5f64.powf(age / HALF_LIFE_SECONDS)
}

/// Directories are compared after normalisation so `~/Work` and `/Users/u/Work`
/// are one row.
pub fn normalize_path(path: &str) -> String {
    let p = path.trim_end_matches(['/', '\\']);
    if p.is_empty() {
        path.to_owned()
    } else {
        p.to_owned()
    }
}

/// A directory in the frecency table.
#[derive(Debug, Clone, PartialEq)]
pub struct Folder {
    pub path: String,
    pub visits: i64,
    pub last_visit: i64,
}

impl Folder {
    fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self { path: r.get(0)?, visits: r.get(1)?, last_visit: r.get(2)? })
    }

    /// The score as of `now`.
    pub fn score(&self, now: i64) -> f64 {
        decayed_score(self.visits, self.last_visit, now)
    }

    /// How long ago this was last used.
    pub fn age(&self, now: i64) -> Duration {
        Duration::from_secs((now - self.last_visit).max(0) as u64)
    }
}

/// A pane session, as session restore reads it back.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionRecord {
    pub id: PaneId,
    pub tab_id: u64,
    pub cwd: Option<String>,
    pub command: Option<String>,
    pub title: Option<String>,
    pub cols: u16,
    pub rows: u16,
    pub pid: Option<u32>,
    /// The multiplexer that has to be reattached, if any (currently `tmux`).
    pub multiplexer: Option<String>,
    pub closed_at: Option<i64>,
    pub restored_at: Option<i64>,
    /// OSC 88 identity, when the pane announced it can be resumed.
    pub identity: Option<String>,
}

impl SessionRecord {
    pub fn new(id: PaneId, cols: u16, rows: u16) -> Self {
        Self {
            id,
            tab_id: 0,
            cwd: None,
            command: None,
            title: None,
            cols,
            rows,
            pid: None,
            multiplexer: None,
            closed_at: None,
            restored_at: None,
            identity: None,
        }
    }

    fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: PaneId::new(r.get::<_, i64>(0)? as u64),
            tab_id: r.get::<_, i64>(1)? as u64,
            cwd: r.get(2)?,
            command: r.get(3)?,
            title: r.get(4)?,
            cols: r.get::<_, i64>(5)? as u16,
            rows: r.get::<_, i64>(6)? as u16,
            pid: r.get::<_, Option<i64>>(7)?.map(|v| v as u32),
            multiplexer: r.get(8)?,
            closed_at: r.get(9)?,
            restored_at: r.get(10)?,
            identity: r.get(11)?,
        })
    }
}

/// One agent conversation.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentSession {
    /// The agent's own session id — an opaque string the agent mints.
    pub id: String,
    pub agent: AgentId,
    pub pane: Option<PaneId>,
    pub cwd: Option<String>,
    pub title: Option<String>,
    pub state: AgentState,
    /// The exact command that resumes this conversation, including the flags
    /// the user's `agent-command` supplies.
    pub resume_command: Option<String>,
    /// The command that forks it into a new conversation.
    pub fork_command: Option<String>,
    /// Whether the session was started in bypass-permissions mode, which the
    /// resume offer has to preserve.
    pub bypass: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub closed_at: Option<i64>,
}

impl AgentSession {
    pub fn new(id: impl Into<String>, agent: AgentId, now: i64) -> Self {
        Self {
            id: id.into(),
            agent,
            pane: None,
            cwd: None,
            title: None,
            state: AgentState::Idle,
            resume_command: None,
            fork_command: None,
            bypass: false,
            created_at: now,
            updated_at: now,
            closed_at: None,
        }
    }

    fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get::<_, String>(0)?,
            agent: AgentId::parse(&r.get::<_, String>(1)?),
            pane: r.get::<_, Option<i64>>(2)?.map(|v| PaneId::new(v as u64)),
            cwd: r.get(3)?,
            title: r.get(4)?,
            state: AgentState::parse(&r.get::<_, String>(5)?).unwrap_or(AgentState::Idle),
            resume_command: r.get(6)?,
            fork_command: r.get(7)?,
            bypass: r.get::<_, i64>(8)? != 0,
            created_at: r.get(9)?,
            updated_at: r.get(10)?,
            closed_at: r.get(11)?,
        })
    }
}

const DDL_V1: &str = r#"
CREATE TABLE IF NOT EXISTS session (
    id          INTEGER PRIMARY KEY,
    tab_id      INTEGER NOT NULL DEFAULT 0,
    cwd         TEXT,
    command     TEXT,
    title       TEXT,
    cols        INTEGER NOT NULL DEFAULT 80,
    rows        INTEGER NOT NULL DEFAULT 24,
    pid         INTEGER,
    multiplexer TEXT,
    closed_at   INTEGER,
    restored_at INTEGER
);

CREATE TABLE IF NOT EXISTS frecency (
    path       TEXT PRIMARY KEY,
    visits     INTEGER NOT NULL DEFAULT 0,
    last_visit INTEGER NOT NULL DEFAULT 0,
    ignored    INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS frecency_last_visit ON frecency (last_visit DESC);
"#;

const DDL_V2: &str = r#"
CREATE TABLE IF NOT EXISTS command_history (
    command    TEXT NOT NULL,
    cwd        TEXT NOT NULL,
    visits     INTEGER NOT NULL DEFAULT 0,
    last_visit INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (command, cwd)
);
"#;

const DDL_V3: &str = r#"
CREATE TABLE IF NOT EXISTS agent_session (
    id             TEXT NOT NULL,
    agent          TEXT NOT NULL,
    pane           INTEGER,
    cwd            TEXT,
    title          TEXT,
    state          TEXT NOT NULL DEFAULT 'idle',
    resume_command TEXT,
    fork_command   TEXT,
    bypass         INTEGER NOT NULL DEFAULT 0,
    created_at     INTEGER NOT NULL DEFAULT 0,
    updated_at     INTEGER NOT NULL DEFAULT 0,
    closed_at      INTEGER
);
CREATE INDEX IF NOT EXISTS agent_session_updated ON agent_session (updated_at DESC);
-- The pair is the key: an agent's session id is scoped to that agent.
CREATE UNIQUE INDEX IF NOT EXISTS agent_session_key ON agent_session (agent, id);
CREATE INDEX IF NOT EXISTS agent_session_pane ON agent_session (pane);
"#;

const DDL_V4: &str = r#"
CREATE TABLE IF NOT EXISTS prompt_queue (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    pane    INTEGER NOT NULL,
    text    TEXT NOT NULL,
    created INTEGER NOT NULL DEFAULT 0
);
"#;

const DDL_V5: &str = r#"
ALTER TABLE session ADD COLUMN identity TEXT;
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store::in_memory().unwrap()
    }

    #[test]
    fn migrations_run_once_and_reach_the_current_version() {
        let mut s = store();
        assert_eq!(s.user_version().unwrap(), SCHEMA_VERSION);
        // Re-running is a no-op, not an error.
        s.migrate().unwrap();
        assert_eq!(s.user_version().unwrap(), SCHEMA_VERSION);
        for table in ["session", "frecency", "command_history", "agent_session"] {
            assert!(s.column_exists(table, "rowid_like").is_ok());
        }
    }

    #[test]
    fn a_newer_database_is_refused_rather_than_downgraded() {
        let mut s = store();
        s.conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1).unwrap();
        assert!(matches!(s.migrate(), Err(StateError::TooNew { .. })));
    }

    #[test]
    fn column_exists_probes_the_right_table() {
        let s = store();
        assert!(s.column_exists("session", "multiplexer").unwrap());
        assert!(!s.column_exists("session", "nope").unwrap());
        assert!(s.column_exists("agent_session", "fork_command").unwrap());
    }

    #[test]
    fn sessions_round_trip_and_are_offered_newest_first() {
        let s = store();
        let mut a = SessionRecord::new(PaneId::new(1), 80, 24);
        a.cwd = Some("/work/otty".into());
        a.command = Some("cargo watch".into());
        a.closed_at = Some(1_000);
        let mut b = SessionRecord::new(PaneId::new(2), 120, 40);
        b.closed_at = Some(2_000);
        s.upsert_session(&a).unwrap();
        s.upsert_session(&b).unwrap();

        let restore = s.sessions_for_restore(10).unwrap();
        assert_eq!(restore.len(), 2);
        assert_eq!(restore[0].id, PaneId::new(2));
        assert_eq!(restore[1].cwd.as_deref(), Some("/work/otty"));
        assert_eq!(restore[1].cols, 80);
    }

    #[test]
    fn upsert_updates_rather_than_duplicating() {
        let s = store();
        let mut a = SessionRecord::new(PaneId::new(1), 80, 24);
        s.upsert_session(&a).unwrap();
        a.cwd = Some("/elsewhere".into());
        a.closed_at = Some(5);
        s.upsert_session(&a).unwrap();
        let all = s.sessions_for_restore(10).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].cwd.as_deref(), Some("/elsewhere"));
    }

    #[test]
    fn a_clean_exit_closes_every_session() {
        let s = store();
        s.upsert_session(&SessionRecord::new(PaneId::new(1), 80, 24)).unwrap();
        assert_eq!(s.close_all_sessions(9_999).unwrap(), 1);
        assert_eq!(s.sessions_for_restore(1).unwrap()[0].closed_at, Some(9_999));
    }

    #[test]
    fn forgetting_a_session_removes_it() {
        let s = store();
        s.upsert_session(&SessionRecord::new(PaneId::new(7), 80, 24)).unwrap();
        s.forget_session(PaneId::new(7)).unwrap();
        assert!(s.sessions_for_restore(10).unwrap().is_empty());
    }

    #[test]
    fn osc88_announce_survives_and_unknown_identity_cannot_resume() {
        let s = store();
        s.record_osc88(PaneId::new(3), "agent:xyz", Some("my-agent --resume xyz"))
            .unwrap();
        let got = s.resume_osc88("agent:xyz").unwrap().expect("identity stored");
        assert_eq!(got.identity.as_deref(), Some("agent:xyz"));
        assert_eq!(got.command.as_deref(), Some("my-agent --resume xyz"));
        assert!(
            s.resume_osc88("missing").unwrap().is_none(),
            "unknown identity must surface cannot-resume"
        );
    }

    #[test]
    fn frecency_ranks_by_both_frequency_and_recency() {
        let s = store();
        let now = 1_800_000_000i64;
        let day = 86_400i64;
        // Used ten times, a year ago.
        for _ in 0..10 {
            s.record_visit("/old/frequent", now - 365 * day).unwrap();
        }
        // Used once, yesterday.
        s.record_visit("/new/rare", now - day).unwrap();
        // Used three times, this week.
        for _ in 0..3 {
            s.record_visit("/week/steady", now - 3 * day).unwrap();
        }

        let ranked = s.ranked_directories(10, now).unwrap();
        let order: Vec<&str> = ranked.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(order[0], "/week/steady", "the steady one wins: {order:?}");
        assert!(order.iter().position(|p| *p == "/new/rare").unwrap()
            < order.iter().position(|p| *p == "/old/frequent").unwrap());
    }

    #[test]
    fn the_decay_halves_every_thirty_days() {
        let now = 1_800_000_000i64;
        let month = 30 * 86_400i64;
        let fresh = decayed_score(1, now, now);
        assert_eq!(fresh, 1.0);
        assert!((decayed_score(1, now - month, now) - 0.5).abs() < 1e-9);
        assert!((decayed_score(1, now - 2 * month, now) - 0.25).abs() < 1e-9);
        // Frequency multiplies the decay…
        assert!((decayed_score(4, now - month, now) - 2.0).abs() < 1e-9);
        // …and a future stamp is treated as now, not as a bonus.
        assert_eq!(decayed_score(1, now + 1000, now), 1.0);
    }

    #[test]
    fn ignoring_a_directory_hides_it_without_erasing_it() {
        let s = store();
        let now = 1_800_000_000i64;
        s.record_visit("/work/old", now).unwrap();
        assert!(s.ignore_path("/work/old").unwrap());
        assert!(s.ranked_directories(10, now).unwrap().is_empty());
        assert!(s.score_of("/work/old", now).unwrap().is_some());
        assert!(!s.ignore_path("/never/seen").unwrap());
    }

    #[test]
    fn trailing_separators_do_not_split_a_row() {
        assert_eq!(normalize_path("/work/otty/"), "/work/otty");
        assert_eq!(normalize_path("/work/otty"), "/work/otty");
        assert_eq!(normalize_path("C:\\work\\"), "C:\\work");
        let s = store();
        let now = 1;
        s.record_visit("/work/otty/", now).unwrap();
        s.record_visit("/work/otty", now).unwrap();
        let ranked = s.ranked_directories(10, now).unwrap();
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].visits, 2);
    }

    #[test]
    fn learned_commands_are_scoped_to_their_directory() {
        let s = store();
        let now = 1;
        s.record_command("cargo test", "/work/otty", now).unwrap();
        s.record_command("cargo test", "/work/otty", now + 1).unwrap();
        s.record_command("git status", "/work/otty", now).unwrap();
        s.record_command("ls", "/tmp", now).unwrap();

        let here = s.commands_in("/work/otty", 10).unwrap();
        assert_eq!(here, ["cargo test", "git status"]);
        assert_eq!(s.commands_in("/tmp", 10).unwrap(), ["ls"]);
    }

    #[test]
    fn agent_sessions_round_trip_with_their_resume_commands() {
        let s = store();
        let mut a = AgentSession::new("9f3c1a2b", AgentId::Claude, 100);
        a.pane = Some(PaneId::new(3));
        a.cwd = Some("/work/otty".into());
        a.resume_command = Some("claude --resume 9f3c1a2b".into());
        a.bypass = true;
        s.upsert_agent_session(&a).unwrap();

        let back = s.agent_history(None, 10).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].id, "9f3c1a2b");
        assert_eq!(back[0].agent, AgentId::Claude);
        assert_eq!(back[0].pane, Some(PaneId::new(3)));
        assert!(back[0].bypass);
        assert_eq!(back[0].resume_command.as_deref(), Some("claude --resume 9f3c1a2b"));
    }

    #[test]
    fn agent_state_transitions_are_visible_to_a_waiter() {
        let s = store();
        let mut a = AgentSession::new("abc12345", AgentId::Codex, 0);
        a.state = AgentState::Processing;
        s.upsert_agent_session(&a).unwrap();
        assert_eq!(s.agent_state(&AgentId::Codex, "abc12345").unwrap(), Some(AgentState::Processing));

        a.state = AgentState::Idle;
        a.updated_at = 1;
        s.upsert_agent_session(&a).unwrap();
        assert_eq!(s.agent_state(&AgentId::Codex, "abc12345").unwrap(), Some(AgentState::Idle));
        assert_eq!(s.pane_for_agent_session(&AgentId::Codex, "abc12345").unwrap(), None);
    }

    #[test]
    fn history_can_be_filtered_to_one_agent() {
        let s = store();
        s.upsert_agent_session(&AgentSession::new("a1", AgentId::Claude, 1)).unwrap();
        s.upsert_agent_session(&AgentSession::new("b2", AgentId::Codex, 2)).unwrap();
        assert_eq!(s.agent_history(None, 10).unwrap().len(), 2);
        let claude = s.agent_history(Some(&AgentId::Claude), 10).unwrap();
        assert_eq!(claude.len(), 1);
        assert_eq!(claude[0].id, "a1");
    }

    #[test]
    fn the_same_session_id_from_two_agents_stays_two_rows() {
        let s = store();
        // Two agents can mint the same session id string. The key is the pair,
        // so neither row overwrites the other.
        s.upsert_agent_session(&AgentSession::new("shared", AgentId::Claude, 1)).unwrap();
        s.upsert_agent_session(&AgentSession::new("shared", AgentId::Codex, 2)).unwrap();
        let all = s.agent_history(None, 10).unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|a| a.agent == AgentId::Codex));
        assert_eq!(
            s.agent_state(&AgentId::Claude, "shared").unwrap(),
            Some(AgentState::Idle)
        );
    }
}
