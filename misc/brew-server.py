#!/usr/bin/env python3
"""
Local BREW server for BlueStation – mit JSON-Konfiguration für Nodes.
Erweitert um Web-UI, Admin-Login, API, Live-Logging, Gruppengesprächs-Tracking
und Echtzeit-Updates via Server-Sent Events (SSE).
"""

import asyncio
import json
import logging
import os
import struct
import time
import uuid
from datetime import datetime, timezone
from typing import Dict, Set, List, Tuple, Optional
from collections import deque

from aiohttp import web, WSMsgType
from aiohttp_session import setup, get_session
from aiohttp_session.cookie_storage import EncryptedCookieStorage
from cryptography import fernet

# ======================================================================
# Konfiguration
LISTEN_HOST = "0.0.0.0"
LISTEN_PORT = 8081
REALM = "brew-router"

# Admin-Login
ADMIN_USER = os.environ.get("BREW_ADMIN_USER", "admin")
ADMIN_PASS = os.environ.get("BREW_ADMIN_PASS", "admin123")

# nodes.json wird nicht mehr für Authentifizierung verwendet
NODES_FILE = os.environ.get("BREW_NODES_FILE", "nodes.json")

MAX_LOG_ENTRIES = 500

FERNET_KEY = os.environ.get("BREW_SESSION_KEY", fernet.Fernet.generate_key().decode())
SESSION_SECRET = FERNET_KEY.encode()

# ======================================================================
# Logging
logging.basicConfig(level=logging.INFO,
                    format="%(asctime)s %(levelname)s %(name)s: %(message)s")
logger = logging.getLogger("brew-server")

# ======================================================================
# Globale Zustände
active_connections: Dict[str, 'ConnectionState'] = {}
sse_clients: List[web.Response] = []  # SSE-Verbindungen für Echtzeit-Updates

stats = {
    "start_time": time.time(),
    "total_registrations": 0,
    "total_calls": 0,
    "total_messages": 0,
    "total_group_calls": 0,
}

log_entries: deque = deque(maxlen=MAX_LOG_ENTRIES)

def add_log_entry(level: str, issi: str, message: str, details: Optional[dict] = None):
    entry = {
        "timestamp": datetime.now(timezone.utc).isoformat(timespec='microseconds') + "Z",
        "level": level,
        "issi": issi,
        "message": message,
        "details": details or {}
    }
    log_entries.append(entry)

# ======================================================================
# Echtzeit-Benachrichtigungen
async def notify_clients(event_type: str, data: dict):
    """Sendet ein SSE-Event an alle verbundenen Clients."""
    if not sse_clients:
        return
    message = f"event: {event_type}\ndata: {json.dumps(data)}\n\n"
    for client in sse_clients[:]:  # Kopie für sichere Iteration
        try:
            await client.write(message.encode())
        except Exception:
            sse_clients.remove(client)

async def notify_update():
    """Sendet ein generisches Update-Event."""
    await notify_clients("update", {
        "timestamp": datetime.now(timezone.utc).isoformat()
    })

async def notify_stats():
    """Sendet aktuelle Statistiken."""
    total_subscribers = sum(len(conn.subscribers) for conn in active_connections.values())
    active_calls = sum(len(conn.active_calls) for conn in active_connections.values())
    active_group_calls = sum(len(conn.group_calls) for conn in active_connections.values())
    await notify_clients("stats", {
        "start_time": stats["start_time"],
        "connections": len(active_connections),
        "subscribers": total_subscribers,
        "active_calls": active_calls,
        "active_group_calls": active_group_calls,
        "total_registrations": stats["total_registrations"],
        "total_calls": stats["total_calls"],
        "total_group_calls": stats["total_group_calls"],
        "total_messages": stats["total_messages"],
        "uptime": int(time.time() - stats["start_time"]),
    })

async def notify_connections():
    """Sendet aktuelle Verbindungsliste."""
    conns = []
    for uuid_str, state in active_connections.items():
        conns.append({
            "uuid": uuid_str,
            "subscribers": list(state.subscribers.keys()),
            "connected_at": state.connected_at.isoformat(),
            "active_calls": len(state.active_calls),
            "group_calls": len(state.group_calls),
        })
    await notify_clients("connections", conns)

async def notify_group_calls():
    """Sendet aktive Gruppengespräche."""
    calls = []
    for conn in active_connections.values():
        for uuid_str, call in conn.group_calls.items():
            calls.append({
                "uuid": uuid_str,
                "source_issi": call.source_issi,
                "dest_gssi": call.dest_gssi,
                "start_time": call.start_time.isoformat(),
                "frame_count": call.frame_count,
                "active": call.active,
            })
    await notify_clients("group_calls", calls)

async def notify_logs():
    """Sendet die neuesten Log-Einträge."""
    entries = list(log_entries)[-50:][::-1]
    await notify_clients("logs", entries)

async def broadcast_full_update():
    """Sendet alle Daten für einen vollständigen Refresh."""
    await notify_stats()
    await notify_connections()
    await notify_group_calls()
    await notify_logs()

# ======================================================================
# Protokollkonstanten
BREW_SUBSCRIBER_DEREGISTER   = 0
BREW_SUBSCRIBER_REGISTER     = 1
BREW_SUBSCRIBER_REREGISTER   = 2
BREW_SUBSCRIBER_AFFILIATE    = 8
BREW_SUBSCRIBER_DEAFFILIATE  = 9

CALL_STATE_GROUP_TX          = 2
CALL_STATE_GROUP_IDLE        = 3
CALL_STATE_SETUP_REQUEST     = 4
CALL_STATE_SETUP_ACCEPT      = 5
CALL_STATE_SETUP_REJECT      = 6
CALL_STATE_CALL_ALERT        = 7
CALL_STATE_CONNECT_REQUEST   = 8
CALL_STATE_CONNECT_CONFIRM   = 9
CALL_STATE_CALL_RELEASE      = 10
CALL_STATE_SHORT_TRANSFER    = 11
CALL_STATE_SIMPLEX_GRANTED   = 12
CALL_STATE_SIMPLEX_IDLE      = 13

FRAME_TYPE_TRAFFIC_CHANNEL   = 0
FRAME_TYPE_SDS_TRANSFER      = 1
FRAME_TYPE_SDS_REPORT        = 2
FRAME_TYPE_DTMF_DATA         = 3
FRAME_TYPE_PACKET_DATA       = 4

BREW_TYPE_MALFORMED          = 0
BREW_TYPE_RESTRICTED         = 1

SERVICE_QUERY_SUBSCRIBERS    = 1
SERVICE_SUBSCRIBER_PROFILES  = 2
SERVICE_ALLOWED_ISSIS_REQUEST = 3
SERVICE_ALLOWED_ISSIS_RESPONSE = 4
SERVICE_RSSI                 = 16

# ======================================================================
# Hilfsfunktionen (Pack/Unpack)
def pack_subscriber_data(type_: int, number: int, time_ns: int, fraction: int,
                         groups: List[int] = None) -> bytes:
    groups = groups or []
    data = struct.pack("<BB I Q I", 0xf0, type_, number, time_ns, fraction)
    for g in groups:
        data += struct.pack("<I", g)
    return data

def pack_call_control_data(type_: int, call_uuid: uuid.UUID, extra: bytes = b"") -> bytes:
    return struct.pack("<BB", 0xf1, type_) + call_uuid.bytes + extra

def pack_service_message(type_: int, json_str: str) -> bytes:
    data = json_str.encode("utf-8") + b"\x00"
    return struct.pack("<BB", 0xf4, type_) + data

def pack_frame_data(type_: int, call_uuid: uuid.UUID, data: bytes, bit_len: int = None) -> bytes:
    if bit_len is None:
        bit_len = len(data) * 8
    return (struct.pack("<BB", 0xf2, type_) + call_uuid.bytes +
            struct.pack("<H", bit_len) + data)

def pack_error_message(type_: int, extra: bytes = b"") -> bytes:
    return struct.pack("<BB", 0xf3, type_) + extra

def unpack_subscriber_data(payload: bytes):
    if len(payload) < 2+4+8+4:
        raise ValueError("Payload too short")
    kind, type_ = struct.unpack_from("<BB", payload, 0)
    if kind != 0xf0:
        raise ValueError(f"Invalid class {kind}")
    number, time_ns, fraction = struct.unpack_from("<I Q I", payload, 2)
    groups_data = payload[2+4+8+4:]
    groups = [struct.unpack_from("<I", groups_data, i)[0]
              for i in range(0, len(groups_data), 4)]
    return type_, number, time_ns, fraction, groups

def unpack_call_control_data(payload: bytes):
    if len(payload) < 2+16:
        raise ValueError("Payload too short")
    kind, type_ = struct.unpack_from("<BB", payload, 0)
    if kind != 0xf1:
        raise ValueError(f"Invalid class {kind}")
    call_uuid = uuid.UUID(bytes=payload[2:18])
    extra = payload[18:]
    source = None
    dest = None
    if type_ == CALL_STATE_GROUP_TX and len(extra) >= 8:
        source = struct.unpack_from("<I", extra, 0)[0]
        dest = struct.unpack_from("<I", extra, 4)[0]
    return type_, call_uuid, extra, source, dest

def unpack_frame_data(payload: bytes):
    if len(payload) < 2+16+2:
        raise ValueError("Payload too short")
    kind, type_ = struct.unpack_from("<BB", payload, 0)
    if kind != 0xf2:
        raise ValueError(f"Invalid class {kind}")
    call_uuid = uuid.UUID(bytes=payload[2:18])
    bit_len = struct.unpack_from("<H", payload, 18)[0]
    return type_, call_uuid, bit_len, payload[20:]

def unpack_service_data(payload: bytes):
    if len(payload) < 2:
        raise ValueError("Payload too short")
    kind, type_ = struct.unpack_from("<BB", payload, 0)
    if kind != 0xf4:
        raise ValueError(f"Invalid class {kind}")
    json_bytes = payload[2:]
    null_pos = json_bytes.find(b"\x00")
    if null_pos < 0:
        raise ValueError("Missing null terminator")
    return type_, json_bytes[:null_pos].decode("utf-8")

# ======================================================================
# ConnectionState
class GroupCall:
    def __init__(self, uuid_str: str, source_issi: int, dest_gssi: int):
        self.uuid = uuid_str
        self.source_issi = source_issi
        self.dest_gssi = dest_gssi
        self.start_time = datetime.now(timezone.utc)
        self.frame_count = 0
        self.active = True

class ConnectionState:
    def __init__(self, websocket: web.WebSocketResponse, uuid_str: str):
        self.websocket = websocket
        self.uuid = uuid_str
        self.subscribers: Dict[int, dict] = {}
        self.group_calls: Dict[str, GroupCall] = {}
        self.active_calls: Dict[str, dict] = {}
        self.connected_at = datetime.now(timezone.utc)
        self.last_activity = datetime.now(timezone.utc)

# ======================================================================
# Nachrichtenbehandlung
async def handle_subscriber_register(state: ConnectionState, number: int):
    logger.info(f"📱 Endgerät REGISTER: ISSI {number}")
    add_log_entry("INFO", str(number), "Endgerät REGISTER")

    if number not in state.subscribers:
        state.subscribers[number] = {
            "registered_at": datetime.now(timezone.utc),
            "groups": set(),
            "last_seen": datetime.now(timezone.utc)
        }
    else:
        state.subscribers[number]["last_seen"] = datetime.now(timezone.utc)

    stats["total_registrations"] += 1

    profile = {
        str(number): {
            "call": f"MS{str(number)[-3:]}",
            "text": f"Mobile {number}",
            "active": True,
            "status": 1,
            "date": datetime.now(timezone.utc).isoformat(timespec='microseconds') + "Z"
        }
    }
    msg = pack_service_message(SERVICE_SUBSCRIBER_PROFILES, json.dumps(profile))
    await state.websocket.send_bytes(msg)
    
    # Echtzeit-Updates
    await notify_stats()
    await notify_connections()
    await notify_logs()

async def handle_subscriber_deregister(state: ConnectionState, number: int):
    logger.info(f"Endgerät DEREGISTER: ISSI {number}")
    add_log_entry("INFO", str(number), "Endgerät DEREGISTER")
    state.subscribers.pop(number, None)
    await notify_stats()
    await notify_connections()
    await notify_logs()

async def handle_subscriber_affiliate(state: ConnectionState, number: int, groups: List[int]):
    logger.info(f"Endgerät {number} affiliated groups {groups}")
    add_log_entry("INFO", str(number), f"AFFILIATE groups={groups}")
    if number in state.subscribers:
        state.subscribers[number]["groups"].update(groups)
    else:
        state.subscribers[number] = {
            "registered_at": datetime.now(timezone.utc),
            "groups": set(groups),
            "last_seen": datetime.now(timezone.utc)
        }
    await notify_connections()
    await notify_logs()

async def handle_group_tx(state: ConnectionState, call_uuid: uuid.UUID, source_issi: int, dest_gssi: int, extra: bytes):
    uuid_str = str(call_uuid)

    if uuid_str in state.group_calls:
        call = state.group_calls[uuid_str]
        if call.source_issi != source_issi:
            call.source_issi = source_issi
            add_log_entry("INFO", str(source_issi), f"Speaker-Wechsel GSSI={dest_gssi}")
        await notify_group_calls()
        return

    stats["total_group_calls"] += 1
    call = GroupCall(uuid_str, source_issi, dest_gssi)
    state.group_calls[uuid_str] = call

    state.active_calls[uuid_str] = {
        "type": "group",
        "source": source_issi,
        "dest": dest_gssi,
        "started": datetime.now(timezone.utc).isoformat(),
        "frames": 0
    }

    logger.info(f"📢 GROUP_TX START: uuid={uuid_str} source={source_issi} dest={dest_gssi}")
    add_log_entry("INFO", str(source_issi), f"GROUP_TX START GSSI={dest_gssi}")
    
    # Echtzeit-Updates
    await notify_stats()
    await notify_group_calls()
    await notify_connections()
    await notify_logs()

async def handle_group_idle(state: ConnectionState, call_uuid: uuid.UUID, cause: int):
    uuid_str = str(call_uuid)

    if uuid_str in state.group_calls:
        call = state.group_calls.pop(uuid_str)
        duration = (datetime.now(timezone.utc) - call.start_time).total_seconds()
        logger.info(f"📢 GROUP_IDLE: uuid={uuid_str} source={call.source_issi} dest={call.dest_gssi} "
                    f"frames={call.frame_count} duration={duration:.1f}s cause={cause}")
        add_log_entry("INFO", str(call.source_issi),
                      f"GROUP_IDLE GSSI={call.dest_gssi} frames={call.frame_count} duration={duration:.1f}s")
        state.active_calls.pop(uuid_str, None)
        
        # Echtzeit-Updates
        await notify_stats()
        await notify_group_calls()
        await notify_connections()
        await notify_logs()
    else:
        logger.debug(f"GROUP_IDLE für unbekanntes uuid={uuid_str}")

async def handle_voice_frame(state: ConnectionState, call_uuid: uuid.UUID, bit_len: int, data: bytes):
    uuid_str = str(call_uuid)

    if uuid_str in state.group_calls:
        call = state.group_calls[uuid_str]
        call.frame_count += 1
        if uuid_str in state.active_calls:
            state.active_calls[uuid_str]["frames"] = call.frame_count
        # Bei jedem 10. Frame ein Update senden (nicht bei jedem)
        if call.frame_count % 10 == 0:
            await notify_group_calls()
    else:
        logger.debug(f"Voice Frame für unbekanntes uuid={uuid_str} (ignoriert)")

async def handle_call_setup_request(state: ConnectionState, call_uuid: uuid.UUID, extra: bytes):
    logger.info(f"Rufaufbau {call_uuid}")
    stats["total_calls"] += 1
    add_log_entry("INFO", "", f"SETUP_REQUEST uuid={call_uuid}")
    await state.websocket.send_bytes(pack_call_control_data(CALL_STATE_SETUP_ACCEPT, call_uuid))
    await notify_stats()
    await notify_logs()

async def handle_connect_request(state: ConnectionState, call_uuid: uuid.UUID, extra: bytes):
    logger.info(f"Connect-Request {call_uuid}")
    add_log_entry("INFO", "", f"CONNECT_REQUEST uuid={call_uuid}")
    grant = struct.pack("<BB", 0, 0)
    await state.websocket.send_bytes(pack_call_control_data(CALL_STATE_CONNECT_CONFIRM, call_uuid, grant))
    state.active_calls[str(call_uuid)] = {"type": "circuit", "state": "connected"}
    await notify_stats()
    await notify_connections()
    await notify_logs()

async def handle_call_release(state: ConnectionState, call_uuid: uuid.UUID, extra: bytes):
    logger.info(f"Ruf freigeben {call_uuid}")
    add_log_entry("INFO", "", f"CALL_RELEASE uuid={call_uuid}")
    state.active_calls.pop(str(call_uuid), None)
    await notify_stats()
    await notify_connections()
    await notify_logs()

async def handle_service_query(state: ConnectionState, json_str: str):
    try:
        issi_list = json.loads(json_str)
        profiles = {}
        for issi in issi_list:
            s = str(issi)
            try:
                issi_int = int(issi)
            except (ValueError, TypeError):
                issi_int = None
            if issi_int is not None and issi_int in state.subscribers:
                profiles[s] = {
                    "call": f"MS{s[-3:]}",
                    "text": f"Mobile {s}",
                    "active": True,
                    "status": 1,
                    "date": datetime.now(timezone.utc).isoformat(timespec='microseconds') + "Z"
                }
            else:
                profiles[s] = {"active": False}
        response = pack_service_message(SERVICE_SUBSCRIBER_PROFILES, json.dumps(profiles))
        await state.websocket.send_bytes(response)
        add_log_entry("DEBUG", "", f"SERVICE_QUERY {issi_list}")
    except Exception as e:
        logger.error(f"Service Query Fehler: {e}")
        add_log_entry("ERROR", "", f"SERVICE_QUERY Fehler: {e}")

async def handle_allowed_issis_request(state: ConnectionState, json_str: str):
    response = {"allowed_issis": list(state.subscribers.keys())}
    msg = pack_service_message(SERVICE_ALLOWED_ISSIS_RESPONSE, json.dumps(response))
    await state.websocket.send_bytes(msg)
    logger.info(f"Sent subscribers to connection {state.uuid}: {list(state.subscribers.keys())}")
    add_log_entry("DEBUG", "", "ALLOWED_ISSIS_REQUEST")

# ======================================================================
# Prozess für binäre Nachrichten
async def process_binary_message(state: ConnectionState, data: bytes):
    if len(data) < 2:
        return
    msg_class = data[0]
    msg_type = data[1]
    stats["total_messages"] += 1

    try:
        if msg_class == 0xf0:
            type_, number, _, _, groups = unpack_subscriber_data(data)
            if type_ == BREW_SUBSCRIBER_REGISTER:
                await handle_subscriber_register(state, number)
            elif type_ == BREW_SUBSCRIBER_DEREGISTER:
                await handle_subscriber_deregister(state, number)
            elif type_ == BREW_SUBSCRIBER_AFFILIATE:
                await handle_subscriber_affiliate(state, number, groups)
            elif type_ == BREW_SUBSCRIBER_REREGISTER:
                await handle_subscriber_register(state, number)
        elif msg_class == 0xf1:
            type_, call_uuid, extra, source, dest = unpack_call_control_data(data)
            if type_ == CALL_STATE_GROUP_TX:
                if source is not None and dest is not None:
                    await handle_group_tx(state, call_uuid, source, dest, extra)
                else:
                    logger.warning(f"GROUP_TX ohne Source/Dest: uuid={call_uuid}")
            elif type_ == CALL_STATE_GROUP_IDLE:
                cause = extra[0] if len(extra) > 0 else 0
                await handle_group_idle(state, call_uuid, cause)
            elif type_ == CALL_STATE_SETUP_REQUEST:
                await handle_call_setup_request(state, call_uuid, extra)
            elif type_ == CALL_STATE_CONNECT_REQUEST:
                await handle_connect_request(state, call_uuid, extra)
            elif type_ == CALL_STATE_CALL_RELEASE:
                await handle_call_release(state, call_uuid, extra)
            else:
                logger.debug(f"Call control type {type_} von Verbindung {state.uuid}")
        elif msg_class == 0xf2:
            type_, call_uuid, bit_len, frame_data = unpack_frame_data(data)
            if type_ == FRAME_TYPE_TRAFFIC_CHANNEL:
                await handle_voice_frame(state, call_uuid, bit_len, frame_data)
            elif type_ == FRAME_TYPE_SDS_TRANSFER:
                logger.info(f"SDS transfer von {state.uuid}: {frame_data.hex()}")
                add_log_entry("INFO", "", f"SDS_TRANSFER {frame_data.hex()[:32]}...")
                await notify_logs()
            elif type_ == FRAME_TYPE_SDS_REPORT:
                status = frame_data[0] if frame_data else 0xff
                logger.info(f"SDS report von {state.uuid}: status {status}")
                add_log_entry("INFO", "", f"SDS_REPORT status={status}")
                await notify_logs()
            elif type_ == FRAME_TYPE_DTMF_DATA:
                dtmf = frame_data.decode('ascii', errors='ignore')
                logger.info(f"DTMF von {state.uuid}: {dtmf}")
                add_log_entry("INFO", "", f"DTMF '{dtmf}'")
                await notify_logs()
            elif type_ == FRAME_TYPE_PACKET_DATA:
                logger.info(f"Packet data von {state.uuid}: {len(frame_data)} bytes")
                add_log_entry("DEBUG", "", f"PACKET_DATA {len(frame_data)} bytes")
        elif msg_class == 0xf3:
            err_type = data[1] if len(data) > 1 else 0
            logger.error(f"Error von {state.uuid}: type {err_type}")
            add_log_entry("ERROR", "", f"ERROR type={err_type}")
            await notify_logs()
        elif msg_class == 0xf4:
            type_, json_str = unpack_service_data(data)
            if type_ == SERVICE_QUERY_SUBSCRIBERS:
                await handle_service_query(state, json_str)
            elif type_ == SERVICE_ALLOWED_ISSIS_REQUEST:
                await handle_allowed_issis_request(state, json_str)
            elif type_ == SERVICE_RSSI:
                logger.debug(f"RSSI von {state.uuid}: {json_str}")
                add_log_entry("DEBUG", "", f"RSSI {json_str}")
            else:
                logger.warning(f"Unbekannter Service-Typ {type_}: {json_str}")
                add_log_entry("WARNING", "", f"UNKNOWN SERVICE type={type_}")
                await notify_logs()
        else:
            logger.warning(f"Unbekannte Nachricht class=0x{msg_class:02x} von {state.uuid}")
            add_log_entry("WARNING", "", f"UNKNOWN CLASS 0x{msg_class:02x}")
            await notify_logs()
    except Exception as e:
        logger.exception(f"Fehler beim Verarbeiten einer Nachricht: {e}")
        add_log_entry("ERROR", "", f"PARSE ERROR: {e}")
        error_msg = pack_error_message(BREW_TYPE_MALFORMED, b"Parse error")
        await state.websocket.send_bytes(error_msg)
        await notify_logs()

# ======================================================================
# WebSocket-Handler
async def websocket_handler(request: web.Request):
    ws_uuid = request.match_info.get("uuid")
    if not ws_uuid:
        return web.HTTPNotFound()

    ws = web.WebSocketResponse(autoclose=True, autoping=True, protocols=["brew"])
    await ws.prepare(request)

    state = ConnectionState(ws, ws_uuid)
    active_connections[ws_uuid] = state

    logger.info(f"🔗 WebSocket-Verbindung aufgebaut: {ws_uuid}")
    add_log_entry("INFO", "", f"WebSocket opened uuid={ws_uuid}")
    await notify_stats()
    await notify_connections()
    await notify_logs()

    try:
        async for msg in ws:
            if msg.type == WSMsgType.BINARY:
                await process_binary_message(state, msg.data)
            elif msg.type == WSMsgType.CLOSE:
                break
            elif msg.type == WSMsgType.ERROR:
                logger.error(f"WebSocket-Fehler: {ws.exception()}")
                break
    finally:
        logger.info(f"WebSocket-Verbindung geschlossen: {ws_uuid}")
        add_log_entry("INFO", "", f"WebSocket closed uuid={ws_uuid}")
        active_connections.pop(ws_uuid, None)
        await notify_stats()
        await notify_connections()
        await notify_logs()

    return ws

# ======================================================================
# HTTP-Endpoints für Web-UI
async def brew_endpoint(request: web.Request):
    ws_uuid = str(uuid.uuid4())
    return web.Response(text=f"/brew/{ws_uuid}")

async def login_page(request: web.Request):
    session = await get_session(request)
    if session.get('logged_in'):
        raise web.HTTPFound('/dashboard')
    html = """
    <!DOCTYPE html>
    <html>
    <head><title>Brew Login</title></head>
    <body style="font-family:sans-serif;display:flex;justify-content:center;align-items:center;height:100vh;background:#f0f0f0;">
        <div style="background:white;padding:40px;border-radius:8px;box-shadow:0 4px 8px rgba(0,0,0,0.1);width:300px;">
            <h2>🍺 Brew Admin</h2>
            <form method="post" action="/login">
                <div style="margin-bottom:15px;">
                    <label>Benutzername</label><br>
                    <input type="text" name="username" style="width:100%;padding:8px;border:1px solid #ccc;border-radius:4px;">
                </div>
                <div style="margin-bottom:15px;">
                    <label>Passwort</label><br>
                    <input type="password" name="password" style="width:100%;padding:8px;border:1px solid #ccc;border-radius:4px;">
                </div>
                <button type="submit" style="width:100%;padding:10px;background:#2196F3;color:white;border:none;border-radius:4px;cursor:pointer;">Login</button>
            </form>
        </div>
    </body>
    </html>
    """
    return web.Response(content_type="text/html", text=html)

async def login_post(request: web.Request):
    data = await request.post()
    username = data.get('username')
    password = data.get('password')
    if username == ADMIN_USER and password == ADMIN_PASS:
        session = await get_session(request)
        session['logged_in'] = True
        raise web.HTTPFound('/dashboard')
    else:
        raise web.HTTPUnauthorized(text="Falsche Anmeldedaten")

async def logout(request: web.Request):
    session = await get_session(request)
    session.clear()
    raise web.HTTPFound('/login')

async def dashboard_page(request: web.Request):
    session = await get_session(request)
    if not session.get('logged_in'):
        raise web.HTTPFound('/login')
    html = """
    <!DOCTYPE html>
    <html>
    <head>
        <meta charset="utf-8">
        <title>Brew Dashboard</title>
        <style>
            body { font-family: sans-serif; margin:20px; background:#f5f5f5; }
            .container { max-width:1200px; margin:0 auto; }
            .card { background:white; border-radius:8px; padding:20px; margin-bottom:20px; box-shadow:0 2px 4px rgba(0,0,0,0.1); }
            .stats { display:grid; grid-template-columns:repeat(auto-fit, minmax(150px,1fr)); gap:15px; }
            .stat { text-align:center; }
            .stat-value { font-size:28px; font-weight:bold; color:#2196F3; }
            .stat-label { font-size:14px; color:#666; }
            table { width:100%; border-collapse:collapse; }
            th, td { padding:10px; text-align:left; border-bottom:1px solid #ddd; }
            th { background:#f0f0f0; }
            .badge { display:inline-block; padding:3px 8px; border-radius:4px; font-size:12px; }
            .badge-active { background:#4CAF50; color:white; }
            .badge-idle { background:#9E9E9E; color:white; }
            .refresh-btn { background:#2196F3; color:white; border:none; padding:10px 20px; border-radius:4px; cursor:pointer; }
            .refresh-btn:hover { background:#1976D2; }
            .footer { text-align:center; color:#999; font-size:12px; margin-top:30px; }
            .log { background:#1e1e1e; color:#d4d4d4; padding:10px; border-radius:4px; font-family:monospace; font-size:12px; max-height:300px; overflow-y:auto; }
            .log .info { color:#4CAF50; }
            .log .warning { color:#FFC107; }
            .log .error { color:#f44336; }
            .log .debug { color:#9E9E9E; }
            .group-call { background:#e3f2fd; padding:8px; border-radius:4px; margin:4px 0; }
            .group-call .source { font-weight:bold; color:#0d47a1; }
            .group-call .dest { color:#1565c0; }
            .group-call .frames { color:#666; font-size:12px; }
            .loading { color:#999; font-style:italic; }
        </style>
    </head>
    <body>
        <div class="container">
            <div style="display:flex;justify-content:space-between;align-items:center;">
                <h1>🍺 Brew Server Dashboard <span style="font-size:14px;color:#4CAF50;">🟢 Echtzeit</span></h1>
                <div>
                    <button class="refresh-btn" onclick="refreshAll()">🔄 Manuell aktualisieren</button>
                    <a href="/logout" style="margin-left:15px;color:#2196F3;text-decoration:none;">Logout</a>
                </div>
            </div>
            <div class="card" id="stats-container">
                <h2>📊 Statistiken</h2>
                <div class="stats" id="stats"><div class="stat"><div class="stat-value">⏳</div><div class="stat-label">Lade...</div></div></div>
            </div>
            <div class="card">
                <h2>📡 Aktive Gruppengespräche</h2>
                <div id="group-calls"><p class="loading">⏳ Lade...</p></div>
            </div>
            <div class="card">
                <h2>🔌 Verbindungen & Endgeräte</h2>
                <table>
                    <thead><tr><th>Verbindungs-UUID</th><th>Endgeräte (ISSI)</th><th>Verbunden seit</th><th>Aktive Anrufe</th></tr></thead>
                    <tbody id="connections"><tr><td colspan="4" class="loading">⏳ Lade...</td></tr></tbody>
                </table>
            </div>
            <div class="card">
                <h2>📋 Logs</h2>
                <div class="log" id="logs"><div class="debug">⏳ Lade...</div></div>
            </div>
            <div class="footer">
                Brew Server v2.0 · <span id="uptime">⏳</span>
            </div>
        </div>
        <script>
            // EventSource für Echtzeit-Updates
            let eventSource = null;

            function connectSSE() {
                if (eventSource) {
                    eventSource.close();
                }
                eventSource = new EventSource('/api/events');

                eventSource.addEventListener('stats', function(e) {
                    const data = JSON.parse(e.data);
                    updateStats(data);
                });

                eventSource.addEventListener('connections', function(e) {
                    const data = JSON.parse(e.data);
                    updateConnections(data);
                });

                eventSource.addEventListener('group_calls', function(e) {
                    const data = JSON.parse(e.data);
                    updateGroupCalls(data);
                });

                eventSource.addEventListener('logs', function(e) {
                    const data = JSON.parse(e.data);
                    updateLogs(data);
                });

                eventSource.addEventListener('update', function(e) {
                    // Generisches Update – wir haben bereits spezifische Events
                });

                eventSource.onerror = function(e) {
                    console.warn('SSE-Fehler, neu verbinden in 3s...', e);
                    setTimeout(connectSSE, 3000);
                };

                eventSource.onopen = function() {
                    console.log('SSE verbunden');
                };
            }

            function updateStats(data) {
                const h = Math.floor(data.uptime / 3600);
                const m = Math.floor((data.uptime % 3600) / 60);
                const s = data.uptime % 60;
                document.getElementById('uptime').textContent = `Laufzeit ${h}h ${m}m ${s}s`;
                document.getElementById('stats').innerHTML = `
                    <div class="stat"><div class="stat-value">${data.connections}</div><div class="stat-label">Verbindungen</div></div>
                    <div class="stat"><div class="stat-value">${data.subscribers}</div><div class="stat-label">Registrierte Endgeräte</div></div>
                    <div class="stat"><div class="stat-value">${data.active_calls}</div><div class="stat-label">Aktive Anrufe</div></div>
                    <div class="stat"><div class="stat-value">${data.active_group_calls}</div><div class="stat-label">Aktive Gruppengespräche</div></div>
                    <div class="stat"><div class="stat-value">${data.total_group_calls}</div><div class="stat-label">Gruppengespräche gesamt</div></div>
                    <div class="stat"><div class="stat-value">${data.total_registrations}</div><div class="stat-label">Registrierungen gesamt</div></div>
                    <div class="stat"><div class="stat-value">${data.total_calls}</div><div class="stat-label">Anrufe gesamt</div></div>
                    <div class="stat"><div class="stat-value">${data.total_messages}</div><div class="stat-label">Nachrichten</div></div>
                `;
            }

            function updateConnections(data) {
                const tbody = document.getElementById('connections');
                if (data.length === 0) {
                    tbody.innerHTML = '<tr><td colspan="4" style="text-align:center;color:#999;">Keine Verbindungen</td></tr>';
                    return;
                }
                tbody.innerHTML = data.map(conn => `
                    <tr>
                        <td>${conn.uuid}</td>
                        <td>${conn.subscribers.join(', ') || '—'}</td>
                        <td>${new Date(conn.connected_at).toLocaleString()}</td>
                        <td>${conn.active_calls}</td>
                    </tr>
                `).join('');
            }

            function updateGroupCalls(data) {
                const container = document.getElementById('group-calls');
                if (data.length === 0) {
                    container.innerHTML = '<p style="color:#999;">Keine aktiven Gruppengespräche</p>';
                    return;
                }
                container.innerHTML = data.map(call => `
                    <div class="group-call">
                        <span class="source">📢 ${call.source_issi}</span>
                        → <span class="dest">GSSI ${call.dest_gssi}</span>
                        <span class="frames">(${call.frame_count} Frames, seit ${new Date(call.start_time).toLocaleTimeString()})</span>
                        <span style="font-size:11px;color:#999;">UUID: ${call.uuid}</span>
                    </div>
                `).join('');
            }

            function updateLogs(data) {
                const container = document.getElementById('logs');
                container.innerHTML = data.map(entry =>
                    `<div class="${entry.level.toLowerCase()}">[${entry.timestamp}] ${entry.issi ? '['+entry.issi+'] ' : ''}${entry.message}</div>`
                ).join('');
                container.scrollTop = container.scrollHeight;
            }

            async function refreshAll() {
                // Manuelles Refresh via API – wird zusätzlich zu SSE verwendet
                try {
                    const [stats, groupCalls, connections, logs] = await Promise.all([
                        fetch('/api/status').then(r => r.json()),
                        fetch('/api/group_calls').then(r => r.json()),
                        fetch('/api/connections').then(r => r.json()),
                        fetch('/api/logs?limit=50').then(r => r.json())
                    ]);
                    updateStats(stats);
                    updateGroupCalls(groupCalls);
                    updateConnections(connections);
                    updateLogs(logs);
                } catch (e) {
                    console.error('Refresh failed', e);
                }
            }

            // SSE verbinden
            connectSSE();

            // Initiales Laden (falls SSE noch keine Daten hat)
            setTimeout(refreshAll, 500);
        </script>
    </body>
    </html>
    """
    return web.Response(content_type="text/html", text=html)

# ======================================================================
# SSE-Endpoint für Echtzeit-Updates
async def sse_endpoint(request: web.Request):
    """Server-Sent Events für Echtzeit-Updates."""
    session = await get_session(request)
    if not session.get('logged_in'):
        raise web.HTTPUnauthorized()

    response = web.StreamResponse()
    response.headers['Content-Type'] = 'text/event-stream'
    response.headers['Cache-Control'] = 'no-cache'
    response.headers['Connection'] = 'keep-alive'
    await response.prepare(request)

    sse_clients.append(response)
    logger.info(f"SSE Client verbunden (aktive: {len(sse_clients)})")

    try:
        # Initiale Daten senden
        await broadcast_full_update()

        # Halte die Verbindung offen
        while True:
            await asyncio.sleep(1)
            # Keep-Alive (alle 15 Sekunden)
            if int(time.time()) % 15 == 0:
                try:
                    await response.write(b": keep-alive\n\n")
                except Exception:
                    break
    except Exception as e:
        logger.debug(f"SSE Client getrennt: {e}")
    finally:
        if response in sse_clients:
            sse_clients.remove(response)
        logger.info(f"SSE Client getrennt (aktive: {len(sse_clients)})")

    return response

# ======================================================================
# API-Endpunkte (für initiales Laden und Fallback)
async def api_status(request: web.Request):
    session = await get_session(request)
    if not session.get('logged_in'):
        raise web.HTTPUnauthorized()
    total_subscribers = sum(len(conn.subscribers) for conn in active_connections.values())
    active_calls = sum(len(conn.active_calls) for conn in active_connections.values())
    active_group_calls = sum(len(conn.group_calls) for conn in active_connections.values())
    return web.json_response({
        "start_time": stats["start_time"],
        "connections": len(active_connections),
        "subscribers": total_subscribers,
        "active_calls": active_calls,
        "active_group_calls": active_group_calls,
        "total_registrations": stats["total_registrations"],
        "total_calls": stats["total_calls"],
        "total_group_calls": stats["total_group_calls"],
        "total_messages": stats["total_messages"],
        "uptime": int(time.time() - stats["start_time"]),
    })

async def api_group_calls(request: web.Request):
    session = await get_session(request)
    if not session.get('logged_in'):
        raise web.HTTPUnauthorized()
    calls = []
    for conn in active_connections.values():
        for uuid_str, call in conn.group_calls.items():
            calls.append({
                "uuid": uuid_str,
                "source_issi": call.source_issi,
                "dest_gssi": call.dest_gssi,
                "start_time": call.start_time.isoformat(),
                "frame_count": call.frame_count,
                "active": call.active,
            })
    return web.json_response(calls)

async def api_connections(request: web.Request):
    session = await get_session(request)
    if not session.get('logged_in'):
        raise web.HTTPUnauthorized()
    conns = []
    for uuid_str, state in active_connections.items():
        conns.append({
            "uuid": uuid_str,
            "subscribers": list(state.subscribers.keys()),
            "connected_at": state.connected_at.isoformat(),
            "active_calls": len(state.active_calls),
            "group_calls": len(state.group_calls),
        })
    return web.json_response(conns)

async def api_logs(request: web.Request):
    session = await get_session(request)
    if not session.get('logged_in'):
        raise web.HTTPUnauthorized()
    limit = int(request.query.get('limit', 100))
    entries = list(log_entries)[-limit:][::-1]
    return web.json_response(entries)

# ======================================================================
# App erstellen
async def init_app():
    app = web.Application()
    fernet_key = fernet.Fernet(SESSION_SECRET)
    setup(app, EncryptedCookieStorage(fernet_key))

    app.router.add_get("/brew/", brew_endpoint)
    app.router.add_get("/brew/{uuid}", websocket_handler)

    app.router.add_get("/login", login_page)
    app.router.add_post("/login", login_post)
    app.router.add_get("/logout", logout)
    app.router.add_get("/dashboard", dashboard_page)

    app.router.add_get("/api/events", sse_endpoint)  # SSE für Echtzeit
    app.router.add_get("/api/status", api_status)
    app.router.add_get("/api/group_calls", api_group_calls)
    app.router.add_get("/api/connections", api_connections)
    app.router.add_get("/api/logs", api_logs)

    return app

def main():
    app = asyncio.run(init_app())
    web.run_app(app, host=LISTEN_HOST, port=LISTEN_PORT)

if __name__ == "__main__":
    main()
