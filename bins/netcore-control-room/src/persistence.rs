use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, params};
use serde_json::{Value, json};
use tetra_core::tetra_entities::TetraEntity;

use crate::config::PersistenceConfig;
use crate::state::{CommandAuditEntry, EmergencyState, EventLogEntry, LocationState, SdsLogEntry};

#[derive(Clone)]
pub struct PersistenceHandle {
    inner: Arc<Mutex<PersistenceInner>>,
}

struct PersistenceInner {
    conn: Connection,
    persist_events: bool,
    persist_noisy_events: bool,
    load_recent_limit: usize,
}

#[derive(Debug, Default)]
pub struct PersistenceBootstrap {
    pub events: Vec<EventLogEntry>,
    pub commands: Vec<CommandAuditEntry>,
    pub sds: Vec<PersistedSdsRow>,
    pub locations: Vec<PersistedLocationRow>,
    pub emergencies: Vec<PersistedEmergencyRow>,
}

#[derive(Debug, Clone)]
pub struct PersistedSdsRow {
    pub node_id: String,
    pub station_name: Option<String>,
    pub entry: SdsLogEntry,
}

#[derive(Debug, Clone)]
pub struct PersistedLocationRow {
    pub node_id: String,
    pub station_name: Option<String>,
    pub issi: u32,
    pub location: LocationState,
}

#[derive(Debug, Clone)]
pub struct PersistedEmergencyRow {
    pub node_id: String,
    pub station_name: Option<String>,
    pub emergency: EmergencyState,
}

impl PersistenceHandle {
    pub fn open(config: &PersistenceConfig) -> rusqlite::Result<Self> {
        if let Some(parent) = config.database_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
            }
        }

        let conn = Connection::open(&config.database_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", 5000_i64)?;
        migrate(&conn)?;

        Ok(Self {
            inner: Arc::new(Mutex::new(PersistenceInner {
                conn,
                persist_events: config.persist_events,
                persist_noisy_events: config.persist_noisy_events,
                load_recent_limit: config.load_recent_limit,
            })),
        })
    }

    pub fn load_bootstrap(&self, history_limit: usize) -> rusqlite::Result<PersistenceBootstrap> {
        let inner = self.inner.lock().expect("persistence mutex poisoned");
        let limit = inner.load_recent_limit.min(history_limit).max(1);
        Ok(PersistenceBootstrap {
            events: load_events(&inner.conn, limit)?,
            commands: load_commands(&inner.conn, limit)?,
            sds: load_sds(&inner.conn, limit)?,
            locations: load_locations(&inner.conn)?,
            emergencies: load_emergencies(&inner.conn, limit)?,
        })
    }

    pub fn persist_node_hello(
        &self,
        node_id: &str,
        station_name: Option<&str>,
        site: Option<&str>,
        timestamp: &str,
        protocol_version: Option<&str>,
        stack_version: Option<&str>,
        raw_hello: &Value,
    ) {
        let raw_hello = raw_hello.to_string();
        self.with_conn("persist node hello", |conn| {
            conn.execute(
                "INSERT INTO node_sessions \
                 (node_id, station_name, site, connected_at, disconnected_at, protocol_version, stack_version, raw_hello) \
                 VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7)",
                params![node_id, station_name, site, timestamp, protocol_version, stack_version, raw_hello],
            )?;
            Ok(())
        });
    }

    pub fn mark_node_disconnected(&self, node_id: &str, timestamp: &str) {
        self.with_conn("mark node disconnected", |conn| {
            conn.execute(
                "UPDATE node_sessions \
                 SET disconnected_at = ?1 \
                 WHERE id = (SELECT id FROM node_sessions WHERE node_id = ?2 AND disconnected_at IS NULL ORDER BY id DESC LIMIT 1)",
                params![timestamp, node_id],
            )?;
            Ok(())
        });
    }

    pub fn persist_event(&self, entry: &EventLogEntry) {
        let persist = {
            let inner = self.inner.lock().expect("persistence mutex poisoned");
            inner.persist_events && (inner.persist_noisy_events || !is_noisy_event_type(&entry.event_type))
        };
        if !persist {
            return;
        }

        let event_json = entry.event.to_string();
        let seq = entry.seq.map(|seq| seq as i64);
        self.with_conn("persist event", |conn| {
            conn.execute(
                "INSERT INTO events (timestamp, node_id, seq, event_type, event_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![entry.timestamp, entry.node_id, seq, entry.event_type, event_json],
            )?;
            Ok(())
        });
    }

    pub fn persist_command(&self, entry: &CommandAuditEntry) {
        let target_entity = serde_json::to_string(&entry.target_entity).unwrap_or_else(|_| "null".to_string());
        let command = entry.command.to_string();
        let responses = serde_json::to_string(&entry.responses).unwrap_or_else(|_| "[]".to_string());
        self.with_conn("persist command", |conn| {
            conn.execute(
                "INSERT INTO commands \
                 (command_id, target_node_id, operator_id, issued_at, updated_at, status, target_entity_json, message, command_json, responses_json) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
                 ON CONFLICT(command_id) DO UPDATE SET \
                   target_node_id = excluded.target_node_id, \
                   operator_id = COALESCE(excluded.operator_id, commands.operator_id), \
                   issued_at = commands.issued_at, \
                   updated_at = excluded.updated_at, \
                   status = excluded.status, \
                   target_entity_json = excluded.target_entity_json, \
                   message = excluded.message, \
                   command_json = CASE WHEN excluded.command_json = 'null' THEN commands.command_json ELSE excluded.command_json END, \
                   responses_json = excluded.responses_json",
                params![
                    entry.command_id,
                    entry.target_node_id,
                    entry.operator_id,
                    entry.issued_at,
                    entry.updated_at,
                    entry.status,
                    target_entity,
                    entry.message,
                    command,
                    responses,
                ],
            )?;
            Ok(())
        });
    }

    pub fn persist_sds(&self, node_id: &str, station_name: Option<&str>, entry: &SdsLogEntry) {
        self.with_conn("persist sds", |conn| {
            conn.execute(
                "INSERT INTO sds_log \
                 (node_id, station_name, timestamp, direction, source_issi, dest_issi, is_group, protocol_id, text) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    node_id,
                    station_name,
                    entry.timestamp,
                    entry.direction,
                    entry.source_issi,
                    entry.dest_issi,
                    if entry.is_group { 1_i64 } else { 0_i64 },
                    entry.protocol_id as i64,
                    entry.text,
                ],
            )?;
            Ok(())
        });
    }

    pub fn persist_location(&self, node_id: &str, station_name: Option<&str>, issi: u32, location: &LocationState) {
        self.with_conn("persist location", |conn| {
            conn.execute(
                "INSERT INTO locations \
                 (node_id, station_name, issi, latitude, longitude, source, updated_at, raw_text) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
                 ON CONFLICT(node_id, issi) DO UPDATE SET \
                   station_name = excluded.station_name, \
                   latitude = excluded.latitude, \
                   longitude = excluded.longitude, \
                   source = excluded.source, \
                   updated_at = excluded.updated_at, \
                   raw_text = excluded.raw_text",
                params![
                    node_id,
                    station_name,
                    issi as i64,
                    location.latitude,
                    location.longitude,
                    location.source,
                    location.updated_at,
                    location.raw_text,
                ],
            )?;
            Ok(())
        });
    }

    pub fn persist_emergency(&self, node_id: &str, station_name: Option<&str>, emergency: &EmergencyState) {
        self.with_conn("persist emergency", |conn| {
            conn.execute(
                "INSERT INTO emergencies \
                 (node_id, station_name, source_issi, dest_ssi, active, raised_at, cleared_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                 ON CONFLICT(node_id, source_issi, raised_at) DO UPDATE SET \
                   station_name = excluded.station_name, \
                   dest_ssi = excluded.dest_ssi, \
                   active = excluded.active, \
                   cleared_at = excluded.cleared_at",
                params![
                    node_id,
                    station_name,
                    emergency.source_issi as i64,
                    emergency.dest_ssi as i64,
                    if emergency.active { 1_i64 } else { 0_i64 },
                    emergency.raised_at,
                    emergency.cleared_at,
                ],
            )?;
            Ok(())
        });
    }

    fn with_conn<F>(&self, label: &str, f: F)
    where
        F: FnOnce(&Connection) -> rusqlite::Result<()>,
    {
        let result = {
            let inner = self.inner.lock().expect("persistence mutex poisoned");
            f(&inner.conn)
        };
        if let Err(err) = result {
            tracing::warn!(%label, "SQLite persistence operation failed: {}", err);
        }
    }
}

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );

        CREATE TABLE IF NOT EXISTS node_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            node_id TEXT NOT NULL,
            station_name TEXT,
            site TEXT,
            connected_at TEXT NOT NULL,
            disconnected_at TEXT,
            protocol_version TEXT,
            stack_version TEXT,
            raw_hello TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_node_sessions_node_time ON node_sessions(node_id, connected_at DESC);

        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            node_id TEXT NOT NULL,
            seq INTEGER,
            event_type TEXT NOT NULL,
            event_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_events_node_time ON events(node_id, timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_events_type_time ON events(event_type, timestamp DESC);

        CREATE TABLE IF NOT EXISTS commands (
            command_id TEXT PRIMARY KEY,
            target_node_id TEXT NOT NULL,
            operator_id TEXT,
            issued_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            status TEXT NOT NULL,
            target_entity_json TEXT,
            message TEXT,
            command_json TEXT NOT NULL,
            responses_json TEXT NOT NULL DEFAULT '[]'
        );
        CREATE INDEX IF NOT EXISTS idx_commands_node_time ON commands(target_node_id, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_commands_status_time ON commands(status, updated_at DESC);

        CREATE TABLE IF NOT EXISTS sds_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            node_id TEXT NOT NULL,
            station_name TEXT,
            timestamp TEXT NOT NULL,
            direction TEXT NOT NULL,
            source_issi INTEGER NOT NULL,
            dest_issi INTEGER NOT NULL,
            is_group INTEGER NOT NULL DEFAULT 0,
            protocol_id INTEGER NOT NULL,
            text TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_sds_node_time ON sds_log(node_id, timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_sds_source_time ON sds_log(source_issi, timestamp DESC);

        CREATE TABLE IF NOT EXISTS locations (
            node_id TEXT NOT NULL,
            station_name TEXT,
            issi INTEGER NOT NULL,
            latitude REAL NOT NULL,
            longitude REAL NOT NULL,
            source TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            raw_text TEXT,
            PRIMARY KEY (node_id, issi)
        );
        CREATE INDEX IF NOT EXISTS idx_locations_time ON locations(updated_at DESC);

        CREATE TABLE IF NOT EXISTS emergencies (
            node_id TEXT NOT NULL,
            station_name TEXT,
            source_issi INTEGER NOT NULL,
            dest_ssi INTEGER NOT NULL,
            active INTEGER NOT NULL,
            raised_at TEXT NOT NULL,
            cleared_at TEXT,
            PRIMARY KEY (node_id, source_issi, raised_at)
        );
        CREATE INDEX IF NOT EXISTS idx_emergencies_active_time ON emergencies(active, raised_at DESC);

        INSERT OR IGNORE INTO schema_migrations(version) VALUES (1);
        "#,
    )
}

fn load_events(conn: &Connection, limit: usize) -> rusqlite::Result<Vec<EventLogEntry>> {
    let mut stmt = conn.prepare(
        "SELECT timestamp, node_id, seq, event_type, event_json FROM events ORDER BY id DESC LIMIT ?1",
    )?;
    let mut rows: Vec<_> = stmt
        .query_map(params![limit as i64], |row| {
            let seq: Option<i64> = row.get(2)?;
            let event_json: String = row.get(4)?;
            Ok(EventLogEntry {
                timestamp: row.get(0)?,
                node_id: row.get(1)?,
                seq: seq.map(|v| v as u64),
                event_type: row.get(3)?,
                event: serde_json::from_str(&event_json).unwrap_or_else(|_| json!({ "error": "event_json parse failed" })),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.reverse();
    Ok(rows)
}

fn load_commands(conn: &Connection, limit: usize) -> rusqlite::Result<Vec<CommandAuditEntry>> {
    let mut stmt = conn.prepare(
        "SELECT command_id, target_node_id, operator_id, issued_at, updated_at, status, \
                target_entity_json, message, command_json, responses_json \
         FROM commands ORDER BY updated_at DESC LIMIT ?1",
    )?;
    let mut rows: Vec<_> = stmt
        .query_map(params![limit as i64], |row| {
            let target_entity_json: Option<String> = row.get(6)?;
            let command_json: String = row.get(8)?;
            let responses_json: String = row.get(9)?;
            Ok(CommandAuditEntry {
                command_id: row.get(0)?,
                target_node_id: row.get(1)?,
                operator_id: row.get(2)?,
                issued_at: row.get(3)?,
                updated_at: row.get(4)?,
                status: row.get(5)?,
                target_entity: target_entity_json
                    .as_deref()
                    .and_then(|s| serde_json::from_str::<Option<TetraEntity>>(s).ok())
                    .flatten(),
                message: row.get(7)?,
                command: serde_json::from_str(&command_json).unwrap_or(Value::Null),
                responses: serde_json::from_str(&responses_json).unwrap_or_default(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.reverse();
    Ok(rows)
}

fn load_sds(conn: &Connection, limit: usize) -> rusqlite::Result<Vec<PersistedSdsRow>> {
    let mut stmt = conn.prepare(
        "SELECT node_id, station_name, timestamp, direction, source_issi, dest_issi, is_group, protocol_id, text \
         FROM sds_log ORDER BY id DESC LIMIT ?1",
    )?;
    let mut rows: Vec<_> = stmt
        .query_map(params![limit as i64], |row| {
            let is_group: i64 = row.get(6)?;
            let protocol_id: i64 = row.get(7)?;
            Ok(PersistedSdsRow {
                node_id: row.get(0)?,
                station_name: row.get(1)?,
                entry: SdsLogEntry {
                    timestamp: row.get(2)?,
                    direction: row.get(3)?,
                    source_issi: row.get::<_, i64>(4)? as u32,
                    dest_issi: row.get::<_, i64>(5)? as u32,
                    is_group: is_group != 0,
                    protocol_id: protocol_id as u8,
                    text: row.get(8)?,
                },
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.reverse();
    Ok(rows)
}

fn load_locations(conn: &Connection) -> rusqlite::Result<Vec<PersistedLocationRow>> {
    let mut stmt = conn.prepare(
        "SELECT node_id, station_name, issi, latitude, longitude, source, updated_at, raw_text \
         FROM locations ORDER BY updated_at DESC",
    )?;
    stmt.query_map([], |row| {
        Ok(PersistedLocationRow {
            node_id: row.get(0)?,
            station_name: row.get(1)?,
            issi: row.get::<_, i64>(2)? as u32,
            location: LocationState {
                latitude: row.get(3)?,
                longitude: row.get(4)?,
                source: row.get(5)?,
                updated_at: row.get(6)?,
                raw_text: row.get(7)?,
            },
        })
    })?
    .collect()
}

fn load_emergencies(conn: &Connection, limit: usize) -> rusqlite::Result<Vec<PersistedEmergencyRow>> {
    let mut stmt = conn.prepare(
        "SELECT node_id, station_name, source_issi, dest_ssi, active, raised_at, cleared_at \
         FROM emergencies ORDER BY raised_at DESC LIMIT ?1",
    )?;
    let mut rows: Vec<_> = stmt
        .query_map(params![limit as i64], |row| {
            let active: i64 = row.get(4)?;
            Ok(PersistedEmergencyRow {
                node_id: row.get(0)?,
                station_name: row.get(1)?,
                emergency: EmergencyState {
                    source_issi: row.get::<_, i64>(2)? as u32,
                    dest_ssi: row.get::<_, i64>(3)? as u32,
                    active: active != 0,
                    raised_at: row.get(5)?,
                    cleared_at: row.get(6)?,
                },
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.reverse();
    Ok(rows)
}

fn is_noisy_event_type(event_type: &str) -> bool {
    matches!(
        event_type,
        "tx_visual" | "tx_quality" | "sdr_health" | "sys_health" | "health_snapshot" | "ms_rssi" | "ts_voice_activity"
    )
}

#[allow(dead_code)]
fn database_exists(path: &Path) -> bool {
    path.exists()
}
