-- NetCore Control Room SQLite schema v2.
-- The service auto-applies this schema on startup; this file is documentation/reference.

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

CREATE TABLE IF NOT EXISTS auth_tokens (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    role TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_used_at TEXT,
    expires_at TEXT,
    created_by TEXT
);
CREATE INDEX IF NOT EXISTS idx_auth_tokens_role ON auth_tokens(role, enabled);
CREATE INDEX IF NOT EXISTS idx_auth_tokens_created ON auth_tokens(created_at DESC);

INSERT OR IGNORE INTO schema_migrations(version) VALUES (1);
INSERT OR IGNORE INTO schema_migrations(version) VALUES (2);
