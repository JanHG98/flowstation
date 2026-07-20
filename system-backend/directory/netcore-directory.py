#!/usr/bin/env python3
"""
NetCore Directory Server
Local RadioID-style registry for NetCore-Tetra.

- No SSL, LAN/local only by design.
- SQLite storage.
- Embedded Web UI.
- RadioID-compatible read endpoints:
    /api/dmr/user/?id=<ISSI>
    /api/dmr/repeater/?id=<ISSI>
- Native registry APIs:
    /api/devices
    /api/basestations
    /api/groups
    /api/device-groups
    /api/status-group-members?issi=<ISSI>
    /api/status
"""

from __future__ import annotations

import argparse
import json
import os
import sqlite3
import time
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse


APP_NAME = "NetCore Directory"
APP_VERSION = "0.2.0"


def now_iso() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat()


def row_to_dict(row: sqlite3.Row) -> dict:
    return {k: row[k] for k in row.keys()}


SCHEMA = """
CREATE TABLE IF NOT EXISTS devices (
    issi INTEGER PRIMARY KEY,
    name TEXT NOT NULL DEFAULT '',
    short TEXT NOT NULL DEFAULT '',
    type TEXT NOT NULL DEFAULT '',
    owner TEXT NOT NULL DEFAULT '',
    role TEXT NOT NULL DEFAULT '',
    icon TEXT NOT NULL DEFAULT 'radio',
    color TEXT NOT NULL DEFAULT 'green',
    visible INTEGER NOT NULL DEFAULT 1,
    notes TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS basestations (
    issi INTEGER PRIMARY KEY,
    name TEXT NOT NULL DEFAULT '',
    short TEXT NOT NULL DEFAULT '',
    location TEXT NOT NULL DEFAULT '',
    mcc TEXT NOT NULL DEFAULT '',
    mnc TEXT NOT NULL DEFAULT '',
    color TEXT NOT NULL DEFAULT 'blue',
    visible INTEGER NOT NULL DEFAULT 1,
    notes TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS groups (
    gssi INTEGER PRIMARY KEY,
    name TEXT NOT NULL DEFAULT '',
    short TEXT NOT NULL DEFAULT '',
    type TEXT NOT NULL DEFAULT '',
    owner TEXT NOT NULL DEFAULT '',
    color TEXT NOT NULL DEFAULT 'teal',
    visible INTEGER NOT NULL DEFAULT 1,
    notes TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL DEFAULT ''
);


CREATE TABLE IF NOT EXISTS device_groups (
    group_id INTEGER PRIMARY KEY,
    opta TEXT NOT NULL DEFAULT '',
    name TEXT NOT NULL DEFAULT '',
    short TEXT NOT NULL DEFAULT '',
    type TEXT NOT NULL DEFAULT 'vehicle',
    owner TEXT NOT NULL DEFAULT '',
    color TEXT NOT NULL DEFAULT 'purple',
    status_sync INTEGER NOT NULL DEFAULT 1,
    visible INTEGER NOT NULL DEFAULT 1,
    notes TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS device_group_members (
    group_id INTEGER NOT NULL,
    issi INTEGER NOT NULL,
    role TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (group_id, issi),
    FOREIGN KEY(group_id) REFERENCES device_groups(group_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS status_messages (
    code INTEGER PRIMARY KEY,
    label TEXT NOT NULL DEFAULT '',
    severity TEXT NOT NULL DEFAULT 'info',
    description TEXT NOT NULL DEFAULT '',
    color TEXT NOT NULL DEFAULT 'blue',
    visible INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL DEFAULT ''
);
"""


TABLES = {
    "devices": {
        "pk": "issi",
        "fields": ["issi", "name", "short", "type", "owner", "role", "icon", "color", "visible", "notes"],
        "required_pk": "issi",
    },
    "basestations": {
        "pk": "issi",
        "fields": ["issi", "name", "short", "location", "mcc", "mnc", "color", "visible", "notes"],
        "required_pk": "issi",
    },
    "groups": {
        "pk": "gssi",
        "fields": ["gssi", "name", "short", "type", "owner", "color", "visible", "notes"],
        "required_pk": "gssi",
    },
    "device_groups": {
        "pk": "group_id",
        "fields": ["group_id", "opta", "name", "short", "type", "owner", "color", "status_sync", "visible", "notes"],
        "required_pk": "group_id",
    },
    "status_messages": {
        "pk": "code",
        "fields": ["code", "label", "severity", "description", "color", "visible"],
        "required_pk": "code",
    },
}


class Store:
    def __init__(self, path: Path):
        self.path = path
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.init_db()

    def conn(self) -> sqlite3.Connection:
        c = sqlite3.connect(self.path)
        c.row_factory = sqlite3.Row
        c.execute("PRAGMA foreign_keys=ON")
        return c

    def init_db(self) -> None:
        with self.conn() as c:
            c.executescript(SCHEMA)
            c.commit()

    def list_items(self, table: str) -> list[dict]:
        meta = TABLES[table]
        pk = meta["pk"]
        with self.conn() as c:
            rows = c.execute(f"SELECT * FROM {table} ORDER BY {pk} ASC").fetchall()
        return [row_to_dict(r) for r in rows]

    def get_item(self, table: str, pk_value: int) -> dict | None:
        meta = TABLES[table]
        pk = meta["pk"]
        with self.conn() as c:
            row = c.execute(f"SELECT * FROM {table} WHERE {pk}=?", (pk_value,)).fetchone()
        return row_to_dict(row) if row else None

    def next_pk(self, table: str) -> int:
        meta = TABLES[table]
        pk = meta["pk"]
        with self.conn() as c:
            row = c.execute(f"SELECT COALESCE(MAX({pk}), 0) + 1 AS next_id FROM {table}").fetchone()
        return int(row["next_id"] if row else 1)

    @staticmethod
    def parse_member_issis(raw) -> list[int]:
        if raw is None:
            return []
        values = []
        if isinstance(raw, str):
            parts = raw.replace("\n", ",").replace(";", ",").split(",")
            values = [p.strip() for p in parts if p.strip()]
        elif isinstance(raw, list):
            values = raw
        else:
            values = [raw]

        out: list[int] = []
        seen: set[int] = set()
        for v in values:
            try:
                if isinstance(v, dict):
                    v = v.get("issi") or v.get("id") or v.get("device_issi")
                issi = int(str(v).strip())
            except Exception:
                continue
            if issi <= 0 or issi in seen:
                continue
            seen.add(issi)
            out.append(issi)
        return out

    def list_device_groups(self) -> list[dict]:
        groups = self.list_items("device_groups")
        for g in groups:
            gid = int(g["group_id"])
            g["members"] = self.list_device_group_members(gid)
            g["member_devices"] = [self.get_item("devices", issi) for issi in g["members"]]
            g["member_devices"] = [d for d in g["member_devices"] if d]
        return groups

    def get_device_group(self, group_id: int) -> dict | None:
        item = self.get_item("device_groups", group_id)
        if not item:
            return None
        item["members"] = self.list_device_group_members(group_id)
        item["member_devices"] = [self.get_item("devices", issi) for issi in item["members"]]
        item["member_devices"] = [d for d in item["member_devices"] if d]
        return item

    def list_device_group_members(self, group_id: int) -> list[int]:
        with self.conn() as c:
            rows = c.execute(
                "SELECT issi FROM device_group_members WHERE group_id=? ORDER BY issi ASC",
                (group_id,),
            ).fetchall()
        return [int(r["issi"]) for r in rows]

    def set_device_group_members(self, group_id: int, members: list[int]) -> None:
        ts = now_iso()
        with self.conn() as c:
            c.execute("DELETE FROM device_group_members WHERE group_id=?", (group_id,))
            for issi in members:
                c.execute(
                    "INSERT OR REPLACE INTO device_group_members (group_id, issi, role, created_at) VALUES (?, ?, '', ?)",
                    (group_id, int(issi), ts),
                )
            c.commit()

    def upsert_device_group(self, data: dict, pk_override: int | None = None) -> dict:
        payload = dict(data)
        has_members = any(k in payload for k in ("members", "member_issis", "members_text"))

        if "members_text" in payload and "members" not in payload:
            payload["members"] = payload["members_text"]
        if "member_issis" in payload and "members" not in payload:
            payload["members"] = payload["member_issis"]

        if pk_override is not None:
            payload["group_id"] = pk_override

        if not payload.get("group_id"):
            payload["group_id"] = self.next_pk("device_groups")

        if "status_sync" in payload:
            payload["status_sync"] = 1 if bool(payload.get("status_sync", True)) else 0

        members = self.parse_member_issis(payload.get("members"))
        payload.pop("members", None)
        payload.pop("member_issis", None)
        payload.pop("members_text", None)

        item = self.upsert_item("device_groups", payload)
        gid = int(item["group_id"])
        if has_members:
            self.set_device_group_members(gid, members)
        return self.get_device_group(gid) or item

    def delete_device_group(self, group_id: int) -> bool:
        with self.conn() as c:
            c.execute("DELETE FROM device_group_members WHERE group_id=?", (group_id,))
            cur = c.execute("DELETE FROM device_groups WHERE group_id=?", (group_id,))
            c.commit()
            return cur.rowcount > 0

    def groups_for_issi(self, issi: int) -> list[dict]:
        with self.conn() as c:
            rows = c.execute(
                """
                SELECT g.* FROM device_groups g
                JOIN device_group_members m ON m.group_id = g.group_id
                WHERE m.issi=? AND g.visible=1
                ORDER BY g.group_id ASC
                """,
                (issi,),
            ).fetchall()
        out = []
        for r in rows:
            g = row_to_dict(r)
            g["members"] = self.list_device_group_members(int(g["group_id"]))
            out.append(g)
        return out

    def delete_item(self, table: str, pk_value: int) -> bool:
        meta = TABLES[table]
        pk = meta["pk"]
        with self.conn() as c:
            cur = c.execute(f"DELETE FROM {table} WHERE {pk}=?", (pk_value,))
            c.commit()
            return cur.rowcount > 0

    def upsert_item(self, table: str, data: dict, pk_override: int | None = None) -> dict:
        meta = TABLES[table]
        pk = meta["pk"]
        fields = meta["fields"]

        item = {}
        for f in fields:
            if f in data:
                item[f] = data[f]

        if pk_override is not None:
            item[pk] = pk_override

        if pk not in item or item.get(pk) in ("", None):
            if table == "device_groups":
                item[pk] = self.next_pk(table)
            else:
                raise ValueError(f"missing {pk}")

        try:
            item[pk] = int(item[pk])
        except Exception:
            raise ValueError(f"{pk} must be numeric")

        if "visible" in fields:
            item["visible"] = 1 if bool(item.get("visible", True)) else 0
        if "status_sync" in fields:
            item["status_sync"] = 1 if bool(item.get("status_sync", True)) else 0

        # Normalize all missing non-PK fields to empty/default-ish values.
        defaults = {
            "name": "",
            "short": "",
            "type": "",
            "owner": "",
            "role": "",
            "icon": "radio",
            "color": "green",
            "location": "",
            "mcc": "",
            "mnc": "",
            "notes": "",
            "label": "",
            "severity": "info",
            "description": "",
            "opta": "",
            "status_sync": 1,
            "visible": 1,
        }

        existing = self.get_item(table, item[pk])
        ts = now_iso()
        if existing:
            base = {f: existing.get(f, defaults.get(f, "")) for f in fields}
            base.update(item)
            base["updated_at"] = ts
            set_cols = [f"{f}=?" for f in fields if f != pk] + ["updated_at=?"]
            vals = [base[f] for f in fields if f != pk] + [base["updated_at"], base[pk]]
            with self.conn() as c:
                c.execute(f"UPDATE {table} SET {', '.join(set_cols)} WHERE {pk}=?", vals)
                c.commit()
            return self.get_item(table, base[pk]) or base

        base = {f: defaults.get(f, "") for f in fields}
        base.update(item)
        base["created_at"] = ts
        base["updated_at"] = ts
        cols = fields + ["created_at", "updated_at"]
        vals = [base.get(c) for c in cols]
        placeholders = ",".join("?" for _ in cols)
        with self.conn() as c:
            c.execute(f"INSERT INTO {table} ({', '.join(cols)}) VALUES ({placeholders})", vals)
            c.commit()
        return self.get_item(table, base[pk]) or base

    def export_all(self) -> dict:
        return {
            "version": 1,
            "exported_at": now_iso(),
            "devices": self.list_items("devices"),
            "basestations": self.list_items("basestations"),
            "groups": self.list_items("groups"),
            "device_groups": self.list_device_groups(),
            "status_messages": self.list_items("status_messages"),
        }

    def import_all(self, payload: dict) -> dict:
        counts = {}

        for key in ("device_groups", "device-groups", "vehicles", "status_groups"):
            items = payload.get(key)
            if not isinstance(items, list):
                continue
            n = 0
            for item in items:
                if isinstance(item, dict):
                    self.upsert_device_group(item)
                    n += 1
            counts["device_groups"] = counts.get("device_groups", 0) + n

        mapping = {
            "devices": "devices",
            "basestations": "basestations",
            "groups": "groups",
            "status_messages": "status_messages",
            "status": "status_messages",
        }
        for key, table in mapping.items():
            items = payload.get(key)
            if not isinstance(items, list):
                continue
            n = 0
            for item in items:
                if isinstance(item, dict):
                    self.upsert_item(table, item)
                    n += 1
            counts[table] = counts.get(table, 0) + n
        return counts


INDEX_HTML = r"""<!doctype html>
<html lang="de">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>NetCore Directory</title>
<style>
:root{
  --bg:#0b111b;--bg2:#111a28;--bg3:#172235;--card:#121d2d;--border:#26364f;
  --text:#eef4ff;--muted:#8fa6c8;--dim:#5d7294;--accent:#19d3ad;--blue:#65a9ff;
  --warn:#ffb84d;--danger:#ff5b78;--ok:#22c59e;--shadow:0 12px 32px rgba(0,0,0,.28);
  --mono:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;--sans:system-ui,-apple-system,Segoe UI,Roboto,sans-serif;
}
*{box-sizing:border-box} body{margin:0;background:radial-gradient(circle at top left,#142540 0,#0b111b 42%,#070b12 100%);color:var(--text);font-family:var(--sans);min-height:100vh}
.app{display:grid;grid-template-columns:260px 1fr;min-height:100vh}
.side{border-right:1px solid var(--border);background:rgba(8,13,22,.76);backdrop-filter:blur(14px);padding:20px;position:sticky;top:0;height:100vh}
.logo{display:flex;gap:12px;align-items:center;margin-bottom:24px}
.logo-icon{width:40px;height:40px;border-radius:12px;background:linear-gradient(135deg,var(--accent),var(--blue));display:grid;place-items:center;color:#001313;font-weight:900;font-family:var(--mono)}
.logo h1{font-size:17px;margin:0}.logo p{font-size:11px;color:var(--dim);margin:2px 0 0;font-family:var(--mono)}
.nav{display:flex;flex-direction:column;gap:8px}.nav button{all:unset;display:flex;justify-content:space-between;align-items:center;padding:12px 13px;border:1px solid transparent;border-radius:12px;color:var(--muted);cursor:pointer}
.nav button:hover{background:rgba(255,255,255,.045);color:var(--text)}.nav button.active{background:rgba(25,211,173,.11);border-color:rgba(25,211,173,.28);color:var(--accent)}
.count{font-family:var(--mono);font-size:11px;background:rgba(255,255,255,.08);padding:2px 8px;border-radius:999px;color:var(--muted)}
.main{padding:24px;min-width:0}.top{display:flex;align-items:flex-start;justify-content:space-between;gap:16px;margin-bottom:18px}
.title h2{font-size:24px;margin:0}.title p{color:var(--muted);margin:5px 0 0}
.actions{display:flex;gap:8px;flex-wrap:wrap}.btn{border:1px solid var(--border);background:var(--bg3);color:var(--text);padding:9px 12px;border-radius:10px;font-weight:700;font-size:12px;cursor:pointer}
.btn:hover{border-color:var(--blue);color:var(--blue)}.btn.primary{background:rgba(25,211,173,.12);border-color:rgba(25,211,173,.45);color:var(--accent)}.btn.danger:hover{border-color:var(--danger);color:var(--danger)}
.grid{display:grid;grid-template-columns:repeat(5,minmax(0,1fr));gap:12px;margin-bottom:18px}.stat{background:rgba(18,29,45,.72);border:1px solid var(--border);border-radius:16px;padding:15px;box-shadow:var(--shadow)}
.stat .k{font:700 10px var(--mono);letter-spacing:.12em;text-transform:uppercase;color:var(--dim)}.stat .v{font:800 26px var(--mono);margin-top:7px}
.panel{background:rgba(18,29,45,.78);border:1px solid var(--border);border-radius:18px;box-shadow:var(--shadow);overflow:hidden}
.panel-head{display:flex;align-items:center;justify-content:space-between;padding:16px;border-bottom:1px solid var(--border);gap:10px}
.search{background:var(--bg);border:1px solid var(--border);color:var(--text);padding:9px 12px;border-radius:10px;min-width:260px}
.table-wrap{overflow:auto}table{width:100%;border-collapse:collapse}th,td{padding:12px 14px;border-bottom:1px solid rgba(255,255,255,.06);text-align:left;font-size:13px}th{color:var(--dim);font:800 10px var(--mono);text-transform:uppercase;letter-spacing:.12em}
tr:hover td{background:rgba(255,255,255,.025)}code{font-family:var(--mono);color:var(--accent);background:rgba(25,211,173,.08);padding:2px 7px;border-radius:7px}
.badge{display:inline-flex;gap:6px;align-items:center;border:1px solid var(--border);border-radius:999px;padding:3px 9px;color:var(--muted);font:800 10px var(--mono);text-transform:uppercase}.badge.ok{border-color:rgba(34,197,158,.4);color:var(--ok);background:rgba(34,197,158,.08)}.badge.warn{border-color:rgba(255,184,77,.4);color:var(--warn);background:rgba(255,184,77,.08)}
.color-dot{display:inline-block;width:9px;height:9px;border-radius:50%;background:var(--accent);box-shadow:0 0 12px currentColor;margin-right:7px}
.modal{position:fixed;inset:0;background:rgba(0,0,0,.62);display:none;align-items:center;justify-content:center;padding:18px}.modal.open{display:flex}
.box{width:min(780px,100%);max-height:90vh;overflow:auto;background:#101a29;border:1px solid var(--border);border-radius:18px;box-shadow:0 20px 70px rgba(0,0,0,.55)}
.box-head{display:flex;justify-content:space-between;align-items:center;padding:16px;border-bottom:1px solid var(--border)}.box-head h3{margin:0}.x{all:unset;font-size:24px;color:var(--muted);cursor:pointer}
.form{padding:16px;display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:12px}.field{display:flex;flex-direction:column;gap:6px}.field.full{grid-column:1/-1}.field label{font:800 10px var(--mono);text-transform:uppercase;letter-spacing:.1em;color:var(--dim)}
input,select,textarea{background:var(--bg);border:1px solid var(--border);color:var(--text);padding:10px;border-radius:10px;font:500 14px var(--sans)}textarea{min-height:88px;resize:vertical}.check{flex-direction:row;align-items:center;margin-top:20px}
.box-actions{display:flex;justify-content:flex-end;gap:8px;padding:16px;border-top:1px solid var(--border)}
.toast{position:fixed;right:18px;bottom:18px;background:#10291f;border:1px solid rgba(25,211,173,.35);color:var(--accent);padding:12px 14px;border-radius:12px;display:none}.toast.show{display:block}
@media(max-width:900px){.app{grid-template-columns:1fr}.side{position:relative;height:auto}.grid{grid-template-columns:repeat(2,1fr)}.form{grid-template-columns:1fr}}
</style>
</head>
<body>
<div class="app">
  <aside class="side">
    <div class="logo"><div class="logo-icon">NC</div><div><h1>NetCore Directory</h1><p id="ver">local · no TLS · LAN</p></div></div>
    <div class="nav">
      <button data-tab="devices" class="active">Geräte <span class="count" id="c-devices">0</span></button>
      <button data-tab="basestations">Basisstationen <span class="count" id="c-basestations">0</span></button>
      <button data-tab="groups">Talkgroups <span class="count" id="c-groups">0</span></button>
      <button data-tab="device_groups">Gerätegruppen <span class="count" id="c-device_groups">0</span></button>
      <button data-tab="status_messages">Statusmeldungen <span class="count" id="c-status_messages">0</span></button>
      <button data-tab="api">API Test <span class="count">curl</span></button>
    </div>
  </aside>
  <main class="main">
    <div class="top">
      <div class="title"><h2 id="page-title">Geräte</h2><p id="page-sub">ISSI-Verzeichnis für HRT/MRT/Gateways.</p></div>
      <div class="actions">
        <button class="btn" onclick="exportAll()">Export</button>
        <button class="btn" onclick="document.getElementById('importFile').click()">Import</button>
        <input id="importFile" type="file" accept=".json,application/json" hidden onchange="importAll(this.files[0])">
        <button class="btn primary" id="newBtn">Neu anlegen</button>
      </div>
    </div>
    <div class="grid">
      <div class="stat"><div class="k">Devices</div><div class="v" id="s-devices">0</div></div>
      <div class="stat"><div class="k">Basestations</div><div class="v" id="s-basestations">0</div></div>
      <div class="stat"><div class="k">Talkgroups</div><div class="v" id="s-groups">0</div></div>
      <div class="stat"><div class="k">Device Groups</div><div class="v" id="s-device_groups">0</div></div>
      <div class="stat"><div class="k">Status</div><div class="v" id="s-status_messages">0</div></div>
    </div>
    <section class="panel" id="table-panel">
      <div class="panel-head"><strong id="panel-title">Geräte</strong><input class="search" id="search" placeholder="Suchen..."></div>
      <div class="table-wrap"><table><thead id="thead"></thead><tbody id="tbody"></tbody></table></div>
    </section>
    <section class="panel" id="api-panel" style="display:none">
      <div class="panel-head"><strong>RadioID-kompatible lokale API</strong></div>
      <div style="padding:16px;line-height:1.7;color:var(--muted)">
        <p><code>/api/dmr/user/?id=2020001</code> → Gerät als RadioID-kompatible Antwort</p>
        <p><code>/api/dmr/repeater/?id=4010001</code> → Basisstation als RadioID-kompatible Antwort</p>
        <p><code>/api/devices</code>, <code>/api/basestations</code>, <code>/api/groups</code>, <code>/api/device-groups</code>, <code>/api/status</code></p>
        <p><code>/api/status-group-members?issi=2020001</code> → Gerätegruppe(n) und Status-Sync-Members zu einer ISSI</p>
        <div style="display:flex;gap:8px;margin-top:14px"><input id="apiId" value="2020001" style="flex:1"><button class="btn primary" onclick="testApi()">Test</button></div>
        <pre id="apiOut" style="margin-top:14px;background:var(--bg);border:1px solid var(--border);border-radius:12px;padding:14px;overflow:auto"></pre>
      </div>
    </section>
  </main>
</div>
<div class="modal" id="modal"><div class="box"><div class="box-head"><h3 id="modalTitle">Eintrag</h3><button class="x" onclick="closeModal()">×</button></div><div class="form" id="form"></div><div class="box-actions"><button class="btn" onclick="closeModal()">Abbrechen</button><button class="btn primary" onclick="saveCurrent()">Speichern</button></div></div></div>
<div class="toast" id="toast"></div>
<script>
const API = {
  devices:'/api/devices', basestations:'/api/basestations', groups:'/api/groups', device_groups:'/api/device-groups', status_messages:'/api/status'
};
const meta = {
  devices:{title:'Geräte',sub:'ISSI-Verzeichnis für HRT/MRT/Gateways.',pk:'issi',cols:['issi','name','short','type','owner','role','visible'],fields:[
    ['issi','ISSI','number'],['name','Name','text'],['short','Kurzname','text'],['type','Typ','text'],['owner','Owner','text'],['role','Rolle','text'],['icon','Icon','text'],['color','Farbe','text'],['visible','Sichtbar','checkbox'],['notes','Notizen','textarea']
  ]},
  basestations:{title:'Basisstationen',sub:'Lokale TBS/BS/Gateway-Verwaltung.',pk:'issi',cols:['issi','name','short','location','mcc','mnc','visible'],fields:[
    ['issi','ISSI','number'],['name','Name','text'],['short','Kurzname','text'],['location','Standort','text'],['mcc','MCC','text'],['mnc','MNC','text'],['color','Farbe','text'],['visible','Sichtbar','checkbox'],['notes','Notizen','textarea']
  ]},
  groups:{title:'Gruppen',sub:'GSSI/Talkgroups mit Namen und Owner.',pk:'gssi',cols:['gssi','name','short','type','owner','visible'],fields:[
    ['gssi','GSSI','number'],['name','Name','text'],['short','Kurzname','text'],['type','Typ','text'],['owner','Owner','text'],['color','Farbe','text'],['visible','Sichtbar','checkbox'],['notes','Notizen','textarea']
  ]},
  device_groups:{title:'Gerätegruppen',sub:'OPTA-/Fahrzeuggruppen: mehrere ISSIs teilen Status und Rückmeldung.',pk:'group_id',cols:['group_id','opta','name','short','type','members','status_sync','visible'],fields:[
    ['group_id','Gruppen-ID','number'],['opta','OPTA / Kennung','text'],['name','Name','text'],['short','Kurzname','text'],['type','Typ','text'],['owner','Owner','text'],['color','Farbe','text'],['status_sync','Status synchronisieren','checkbox'],['visible','Sichtbar','checkbox'],['members','Mitglieder ISSI (Komma oder Zeilen)','textarea'],['notes','Notizen','textarea']
  ]},
  status_messages:{title:'Statusmeldungen',sub:'Statuscodes und Labels.',pk:'code',cols:['code','label','severity','description','visible'],fields:[
    ['code','Code','number'],['label','Label','text'],['severity','Severity','text'],['description','Beschreibung','textarea'],['color','Farbe','text'],['visible','Sichtbar','checkbox']
  ]}
};
let data={}, tab='devices', editing=null;
function esc(s){return String(s??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]))}
function toast(t){let e=document.getElementById('toast');e.textContent=t;e.classList.add('show');setTimeout(()=>e.classList.remove('show'),1800)}
async function load(){
  for(const k of Object.keys(API)){
    const r=await fetch(API[k],{cache:'no-store'}); data[k]=await r.json();
  }
  renderCounts(); render();
}
function renderCounts(){for(const k of Object.keys(API)){let n=(data[k]||[]).length;document.getElementById('c-'+k).textContent=n;document.getElementById('s-'+k).textContent=n}}
function render(){
  document.querySelectorAll('.nav button').forEach(b=>b.classList.toggle('active',b.dataset.tab===tab));
  let api=tab==='api';document.getElementById('table-panel').style.display=api?'none':'';document.getElementById('api-panel').style.display=api?'':'none';document.getElementById('newBtn').style.display=api?'none':'';
  if(api){document.getElementById('page-title').textContent='API Test';document.getElementById('page-sub').textContent='Lokale RadioID-kompatible Endpunkte.';return}
  let m=meta[tab];document.getElementById('page-title').textContent=m.title;document.getElementById('page-sub').textContent=m.sub;document.getElementById('panel-title').textContent=m.title;
  document.getElementById('thead').innerHTML='<tr>'+m.cols.map(c=>`<th>${esc(c)}</th>`).join('')+'<th>Aktion</th></tr>';
  let q=document.getElementById('search').value.toLowerCase().trim();
  let rows=(data[tab]||[]).filter(x=>!q||JSON.stringify(x).toLowerCase().includes(q));
  document.getElementById('tbody').innerHTML=rows.map(x=>'<tr>'+m.cols.map(c=>cell(c,x[c],x)).join('')+`<td><button class="btn" onclick='openModal(${JSON.stringify(x)})'>Edit</button> <button class="btn danger" onclick="delItem(${x[m.pk]})">Del</button></td></tr>`).join('');
}
function cell(c,v,row){
  if(c==='issi'||c==='gssi'||c==='code')return `<td><code>${esc(v)}</code></td>`;
  if(c==='visible')return `<td><span class="badge ${v?'ok':'warn'}">${v?'visible':'hidden'}</span></td>`;
  if(c==='status_sync')return `<td><span class="badge ${v?'ok':'warn'}">${v?'sync':'solo'}</span></td>`;
  if(c==='members')return `<td>${Array.isArray(v)?v.map(x=>`<code>${esc(x)}</code>`).join(' '):esc(v)}</td>`;
  if(c==='color')return `<td><span class="color-dot" style="background:${esc(v)}"></span>${esc(v)}</td>`;
  return `<td>${esc(v)}</td>`;
}
function openModal(item=null){
  editing=item; let m=meta[tab]; document.getElementById('modalTitle').textContent=item?'Eintrag bearbeiten':'Eintrag anlegen';
  document.getElementById('form').innerHTML=m.fields.map(([k,l,t])=>{
    let val=item?item[k]:'';
    if(t==='checkbox')return `<label class="field check"><input id="f-${k}" type="checkbox" ${val===0||val===false?'':'checked'}> ${l}</label>`;
    if(t==='textarea')return `<label class="field full"><span>${l}</span><textarea id="f-${k}">${esc(val)}</textarea></label>`;
    return `<label class="field"><span>${l}</span><input id="f-${k}" type="${t}" value="${esc(val)}"></label>`;
  }).join('');
  document.getElementById('modal').classList.add('open');
}
function closeModal(){document.getElementById('modal').classList.remove('open')}
async function saveCurrent(){
  let m=meta[tab], obj={}; for(const [k,l,t] of m.fields){let e=document.getElementById('f-'+k);obj[k]=t==='checkbox'?e.checked:e.value}
  let pk=obj[m.pk]; let url=API[tab]+(editing?('/'+editing[m.pk]):'');
  let method=editing?'PUT':'POST';
  let r=await fetch(url,{method,headers:{'Content-Type':'application/json'},body:JSON.stringify(obj)});
  if(!r.ok){toast(await r.text());return}
  closeModal();toast('Gespeichert');await load();
}
async function delItem(pk){
  if(!confirm('Wirklich löschen?'))return;
  let r=await fetch(API[tab]+'/'+pk,{method:'DELETE'});
  if(!r.ok){toast(await r.text());return}
  toast('Gelöscht');await load();
}
async function exportAll(){let r=await fetch('/api/export');let j=await r.json();let blob=new Blob([JSON.stringify(j,null,2)],{type:'application/json'});let a=document.createElement('a');a.href=URL.createObjectURL(blob);a.download='netcore-directory-export.json';a.click()}
async function importAll(file){if(!file)return;let txt=await file.text();let r=await fetch('/api/import',{method:'POST',headers:{'Content-Type':'application/json'},body:txt});toast(r.ok?'Import OK':await r.text());await load()}
async function testApi(){let id=document.getElementById('apiId').value.trim();let r=await fetch('/api/dmr/user/?id='+encodeURIComponent(id));document.getElementById('apiOut').textContent=JSON.stringify(await r.json(),null,2)}
document.querySelectorAll('.nav button').forEach(b=>b.onclick=()=>{tab=b.dataset.tab;render()});
document.getElementById('newBtn').onclick=()=>openModal();
document.getElementById('search').oninput=render;
load();
</script>
</body>
</html>
"""


class Handler(BaseHTTPRequestHandler):
    store: Store = None  # type: ignore

    def log_message(self, fmt: str, *args) -> None:
        print(f"{self.address_string()} - {fmt % args}")

    def send_text(self, status: int, body: str, content_type: str = "text/plain; charset=utf-8") -> None:
        data = body.encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(data)

    def send_json(self, status: int, obj) -> None:
        self.send_text(status, json.dumps(obj, ensure_ascii=False), "application/json; charset=utf-8")

    def read_json(self) -> dict:
        n = int(self.headers.get("Content-Length", "0") or "0")
        if n <= 0:
            return {}
        raw = self.rfile.read(min(n, 2 * 1024 * 1024))
        return json.loads(raw.decode("utf-8"))

    def do_GET(self) -> None:
        parsed = urlparse(self.path)
        path = parsed.path.rstrip("/") or "/"
        parts = [p for p in path.split("/") if p]

        if path == "/":
            self.send_text(200, INDEX_HTML, "text/html; charset=utf-8")
            return

        if path == "/api/health":
            self.send_json(200, {"ok": True, "name": APP_NAME, "version": APP_VERSION, "time": now_iso()})
            return

        if path == "/api/export":
            self.send_json(200, self.store.export_all())
            return

        # RadioID-compatible local endpoints.
        if path == "/api/dmr/user":
            q = parse_qs(parsed.query)
            issi = self.parse_int(q.get("id", ["0"])[0])
            self.send_json(200, self.radioid_device_response(issi))
            return

        if path == "/api/dmr/repeater":
            q = parse_qs(parsed.query)
            issi = self.parse_int(q.get("id", ["0"])[0])
            self.send_json(200, self.radioid_repeater_response(issi))
            return

        if path == "/api/status-group-members":
            q = parse_qs(parsed.query)
            issi = self.parse_int(q.get("issi", q.get("id", ["0"]))[0])
            groups = self.store.groups_for_issi(issi)
            members: list[int] = []
            seen: set[int] = set()
            for g in groups:
                if not g.get("status_sync", 1):
                    continue
                for m in g.get("members", []):
                    if m not in seen:
                        seen.add(m)
                        members.append(m)
            self.send_json(200, {"issi": issi, "count": len(groups), "groups": groups, "status_sync_members": members})
            return

        route = self.resolve_collection(parts)
        if route:
            table, pk = route
            if table == "device_groups":
                if pk is None:
                    self.send_json(200, self.store.list_device_groups())
                else:
                    item = self.store.get_device_group(pk)
                    self.send_json(200 if item else 404, item or {"error": "not found"})
            elif pk is None:
                self.send_json(200, self.store.list_items(table))
            else:
                item = self.store.get_item(table, pk)
                self.send_json(200 if item else 404, item or {"error": "not found"})
            return

        self.send_json(404, {"error": "not found"})

    def do_POST(self) -> None:
        parsed = urlparse(self.path)
        parts = [p for p in parsed.path.rstrip("/").split("/") if p]

        if parsed.path.rstrip("/") == "/api/import":
            try:
                payload = self.read_json()
                counts = self.store.import_all(payload)
                self.send_json(200, {"ok": True, "counts": counts})
            except Exception as e:
                self.send_json(400, {"ok": False, "error": str(e)})
            return

        route = self.resolve_collection(parts)
        if route:
            table, pk = route
            try:
                if table == "device_groups":
                    item = self.store.upsert_device_group(self.read_json(), pk)
                else:
                    item = self.store.upsert_item(table, self.read_json(), pk)
                self.send_json(200, item)
            except Exception as e:
                self.send_json(400, {"error": str(e)})
            return

        self.send_json(404, {"error": "not found"})

    def do_PUT(self) -> None:
        self.do_POST()

    def do_PATCH(self) -> None:
        self.do_POST()

    def do_DELETE(self) -> None:
        parsed = urlparse(self.path)
        parts = [p for p in parsed.path.rstrip("/").split("/") if p]
        route = self.resolve_collection(parts)
        if route:
            table, pk = route
            if pk is None:
                self.send_json(400, {"error": "missing id"})
                return
            if table == "device_groups":
                ok = self.store.delete_device_group(pk)
            else:
                ok = self.store.delete_item(table, pk)
            self.send_json(200 if ok else 404, {"ok": ok})
            return
        self.send_json(404, {"error": "not found"})

    @staticmethod
    def parse_int(v) -> int:
        try:
            return int(str(v).strip())
        except Exception:
            return 0

    def resolve_collection(self, parts: list[str]) -> tuple[str, int | None] | None:
        if len(parts) < 2 or parts[0] != "api":
            return None
        name = parts[1]
        alias = {
            "devices": "devices",
            "basestations": "basestations",
            "base-stations": "basestations",
            "groups": "groups",
            "device-groups": "device_groups",
            "device_groups": "device_groups",
            "status-groups": "device_groups",
            "vehicles": "device_groups",
            "status": "status_messages",
            "status_messages": "status_messages",
        }.get(name)
        if not alias:
            return None
        pk = None
        if len(parts) >= 3:
            pk = self.parse_int(parts[2])
        return alias, pk

    def radioid_device_response(self, issi: int) -> dict:
        if not issi:
            return {"count": 0, "results": []}
        item = self.store.get_item("devices", issi)
        if not item or not item.get("visible", 1):
            return {"count": 0, "results": []}
        callsign = item.get("short") or item.get("name") or str(issi)
        return {
            "count": 1,
            "results": [{
                "id": issi,
                "callsign": callsign,
                "fname": item.get("owner", ""),
                "surname": item.get("type", ""),
                "city": item.get("role", ""),
                "state": item.get("name", ""),
                "country": "NetCore",
                "remarks": item.get("notes", ""),
                "netcore": item,
            }],
        }

    def radioid_repeater_response(self, issi: int) -> dict:
        if not issi:
            return {"count": 0, "results": []}
        item = self.store.get_item("basestations", issi)
        if not item or not item.get("visible", 1):
            return {"count": 0, "results": []}
        callsign = item.get("short") or item.get("name") or str(issi)
        return {
            "count": 1,
            "results": [{
                "id": issi,
                "callsign": callsign,
                "city": item.get("location", ""),
                "state": item.get("mcc", ""),
                "country": "NetCore",
                "remarks": item.get("notes", ""),
                "netcore": item,
            }],
        }


def seed_from_file(store: Store, seed_path: Path) -> None:
    if not seed_path.exists():
        return
    with seed_path.open("r", encoding="utf-8") as f:
        payload = json.load(f)

    # Accept simple devices.json format: {"2010002": {"name": ...}}
    if isinstance(payload, dict) and "devices" not in payload and all(str(k).isdigit() for k in payload.keys()):
        items = []
        for issi, item in payload.items():
            if isinstance(item, str):
                items.append({"issi": int(issi), "name": item})
            elif isinstance(item, dict):
                x = dict(item)
                x["issi"] = int(issi)
                items.append(x)
        store.import_all({"devices": items})
    elif isinstance(payload, dict):
        store.import_all(payload)


def main() -> None:
    ap = argparse.ArgumentParser(description=APP_NAME)
    ap.add_argument("--host", default="0.0.0.0")
    ap.add_argument("--port", type=int, default=8095)
    ap.add_argument("--db", default="./netcore_directory.db")
    ap.add_argument("--seed", default="", help="Optional JSON seed/import file")
    args = ap.parse_args()

    store = Store(Path(args.db))
    if args.seed:
        seed_from_file(store, Path(args.seed))

    Handler.store = store
    srv = ThreadingHTTPServer((args.host, args.port), Handler)
    print(f"{APP_NAME} {APP_VERSION} listening on http://{args.host}:{args.port}")
    print(f"DB: {Path(args.db).resolve()}")
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        print("\nbye")


if __name__ == "__main__":
    main()
