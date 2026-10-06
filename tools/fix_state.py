"""otty-state: agent session ids are opaque agent-scoped strings, and the store
must key on the pair (agent, id), not the id alone."""
import pathlib
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\state\src\lib.rs")
t = F.read_text(encoding="utf-8")
before = t

REPL = [
    # imports: SessionId is not an agent-scoped key
    ("use otty_core::{AgentId, AgentState, PaneId, SessionId};",
     "use otty_core::{AgentId, AgentState, PaneId};"),

    # --- record ------------------------------------------------------------
    ("""    /// Record or update an agent conversation.
    pub fn upsert_agent_session(&self, s: &AgentSession) -> Result<(), StateError> {
        self.conn.execute(
            "INSERT INTO agent_session
                 (id, agent, pane, cwd, title, state, resume_command, fork_command, bypass,
                  created_at, updated_at, closed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET""",
     """    /// Record or update an agent conversation.
    ///
    /// The key is `(agent, id)`: an agent's session id is unique within that
    /// agent but two agents can mint the same string, and a hook names both.
    pub fn upsert_agent_session(&self, s: &AgentSession) -> Result<(), StateError> {
        self.conn.execute(
            "INSERT INTO agent_session
                 (id, agent, pane, cwd, title, state, resume_command, fork_command, bypass,
                  created_at, updated_at, closed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(agent, id) DO UPDATE SET"""),

    ('                s.id.as_str(),\n', '                s.id,\n'),

    # --- state query -------------------------------------------------------
    ("""    /// The live state of one agent session, which is what `otty watch:<agent>`
    /// polls.
    pub fn agent_state(&self, id: &SessionId) -> Result<Option<AgentState>, StateError> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT state FROM agent_session WHERE id = ?1",
                params![id.as_str()],
                |r| r.get(0),
            )
            .optional()?;
        Ok(raw.as_deref().and_then(AgentState::parse))
    }

    /// Which pane an agent session is attached to, so an incoming hook can be
    /// matched to a pane before a session mapping exists.
    pub fn pane_for_agent_session(&self, id: &SessionId) -> Result<Option<PaneId>, StateError> {
        let raw: Option<i64> = self
            .conn
            .query_row(
                "SELECT pane FROM agent_session WHERE id = ?1",
                params![id.as_str()],
                |r| r.get(0),
            )
            .optional()?;
        Ok(raw.map(|v| PaneId::new(v as u64)))
    }""",
     """    /// The live state of one agent session, which is what `otty watch:<agent>`
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
        let raw: Option<i64> = self
            .conn
            .query_row(
                "SELECT pane FROM agent_session WHERE agent = ?1 AND id = ?2",
                params![agent.as_str(), id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(raw.map(|v| PaneId::new(v as u64)))
    }"""),

    # --- model -------------------------------------------------------------
    ("""pub struct AgentSession {
    pub id: SessionId,""",
     """pub struct AgentSession {
    /// The agent's own session id — an opaque string the agent mints.
    pub id: String,"""),

    ("""impl AgentSession {
    pub fn new(id: SessionId, agent: AgentId, now: i64) -> Self {
        Self {
            id,""",
     """impl AgentSession {
    pub fn new(id: impl Into<String>, agent: AgentId, now: i64) -> Self {
        Self {
            id: id.into(),"""),

    ('            id: SessionId::new(0).with_slug(r.get::<_, String>(0)?),',
     '            id: r.get::<_, String>(0)?,'),

    # --- DDL ---------------------------------------------------------------
    ("CREATE TABLE IF NOT EXISTS agent_session (\n    id             TEXT PRIMARY KEY,",
     "CREATE TABLE IF NOT EXISTS agent_session (\n    id             TEXT NOT NULL,"),
    ("CREATE INDEX IF NOT EXISTS agent_session_updated ON agent_session (updated_at DESC);",
     "CREATE INDEX IF NOT EXISTS agent_session_updated ON agent_session (updated_at DESC);\n-- The pair is the key: an agent's session id is scoped to that agent.\nCREATE UNIQUE INDEX IF NOT EXISTS agent_session_key ON agent_session (agent, id);"),
]

for old, new in REPL:
    if old not in t:
        print(f"MISS: {old.splitlines()[0][:90]}")
        sys.exit(1)
    t = t.replace(old, new, 1)

# --- tests ------------------------------------------------------------------
TESTS = [
    ('AgentSession::new(SessionId::new(0).with_slug("9f3c1a2b"), AgentId::Claude, 100)',
     'AgentSession::new("9f3c1a2b", AgentId::Claude, 100)'),
    ('assert_eq!(back[0].id.as_str(), "9f3c1a2b");', 'assert_eq!(back[0].id, "9f3c1a2b");'),
    ('let id = SessionId::new(0).with_slug("abc12345");\n        let mut a = AgentSession::new(id, AgentId::Codex, 0);',
     'let mut a = AgentSession::new("abc12345", AgentId::Codex, 0);'),
    ('assert_eq!(s.agent_state(&id).unwrap(), Some(AgentState::Processing));',
     'assert_eq!(s.agent_state(&AgentId::Codex, "abc12345").unwrap(), Some(AgentState::Processing));'),
    ('assert_eq!(s.agent_state(&id).unwrap(), Some(AgentState::Idle));',
     'assert_eq!(s.agent_state(&AgentId::Codex, "abc12345").unwrap(), Some(AgentState::Idle));'),
    ('assert_eq!(s.pane_for_agent_session(&id).unwrap(), None);',
     'assert_eq!(s.pane_for_agent_session(&AgentId::Codex, "abc12345").unwrap(), None);'),
    ('s.upsert_agent_session(&AgentSession::new(SessionId::new(0).with_slug("a1"), AgentId::Claude, 1))\n            .unwrap();',
     's.upsert_agent_session(&AgentSession::new("a1", AgentId::Claude, 1)).unwrap();'),
    ('s.upsert_agent_session(&AgentSession::new(SessionId::new(0).with_slug("b2"), AgentId::Codex, 2))\n            .unwrap();',
     's.upsert_agent_session(&AgentSession::new("b2", AgentId::Codex, 2)).unwrap();'),
    ('assert_eq!(claude[0].id.as_str(), "a1");', 'assert_eq!(claude[0].id, "a1");'),
    ("""    #[test]
    fn the_same_session_id_from_two_agents_stays_two_rows() {
        let s = store();
        let claude = SessionId::new(0).with_slug("shared");
        let codex = SessionId::new(0).with_slug("shared");
        s.upsert_agent_session(&AgentSession::new(claude, AgentId::Claude, 1)).unwrap();
        s.upsert_agent_session(&AgentSession::new(codex, AgentId::Codex, 2)).unwrap();
        // The primary key is the agent's own session id, which is unique per
        // agent but not across agents — so the store keys on the pair.
        let all = s.agent_history(None, 10).unwrap();
        assert!(all.iter().any(|a| a.agent == AgentId::Codex));
    }""",
     """    #[test]
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
    }"""),
]

for old, new in TESTS:
    if old not in t:
        print(f"MISS(test): {old.splitlines()[0][:90]}")
        sys.exit(1)
    t = t.replace(old, new, 1)

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched otty-state")
