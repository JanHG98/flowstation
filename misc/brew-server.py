#!/usr/bin/env python3
"""
Brew Server - TETRA Homebrew Protocol Implementation
Nach Spezifikation: https://wiki.tetrapack.online/books/tetra/page/brew

Dieser Server implementiert:
- HTTP Digest Authentication (RFC 2831)
- WebSocket (RFC 6455) mit binären Nachrichten
- Subscriber Control (Klasse 0xf0)
- Call Control (Klasse 0xf1)
- Einfaches Web-Dashboard
"""

import asyncio
import hashlib
import json
import struct
import time
import uuid
from datetime import datetime
from http.server import HTTPServer, BaseHTTPRequestHandler
from socketserver import ThreadingMixIn
import threading
import urllib.parse

# WebSocket-Bibliothek
try:
    import websockets
except ImportError:
    print("Fehler: Bitte installiere websockets: pip install websockets")
    exit(1)

# ============================================================
# KONFIGURATION
# ============================================================
CONFIG = {
    "http_port": 8080,
    "ws_port": 8081,
    "realm": "BrewServer",
    "username": "brew",
    "password": "brew123",  # In Produktion ändern!
    "server_name": "PythonBrew/1.0",
}

# ============================================================
# BREW PROTOKOLL KONSTANTEN
# ============================================================
# Message Classes
BREW_SUBSCRIBER_CONTROL = 0xf0
BREW_CALL_CONTROL = 0xf1
BREW_FRAME_DATA = 0xf2
BREW_ERROR = 0xf3
BREW_SERVICE = 0xf4

# Subscriber Control Types (Klasse 0xf0)
BREW_SUBSCRIBER_DEREGISTER = 0
BREW_SUBSCRIBER_REGISTER = 1
BREW_SUBSCRIBER_REREGISTER = 2
BREW_SUBSCRIBER_AFFILIATE = 8
BREW_SUBSCRIBER_DEAFFILIATE = 9

# Call Control Types (Klasse 0xf1)
CALL_STATE_GROUP_TX = 2
CALL_STATE_GROUP_IDLE = 3
CALL_STATE_SETUP_REQUEST = 4
CALL_STATE_SETUP_ACCEPT = 5
CALL_STATE_SETUP_REJECT = 6
CALL_STATE_CALL_ALERT = 7
CALL_STATE_CONNECT_REQUEST = 8
CALL_STATE_CONNECT_CONFIRM = 9
CALL_STATE_CALL_RELEASE = 10

# ============================================================
# GLOBALER STATE
# ============================================================
class BrewState:
    """Hält den gesamten Zustand des Brew-Servers."""
    def __init__(self):
        self.subscribers = {}  # ISSI -> {info}
        self.affiliations = {}  # ISSI -> [GSSI-Liste]
        self.active_calls = []  # Aktive Anrufe
        self.connections = []   # Offene WebSocket-Verbindungen
        self.stats = {
            "start_time": time.time(),
            "total_registrations": 0,
            "total_calls": 0,
        }
        self._lock = asyncio.Lock()

    async def add_subscriber(self, issi: int, groups: list = None):
        """Registriert einen Teilnehmer."""
        async with self._lock:
            if issi not in self.subscribers:
                self.stats["total_registrations"] += 1
            self.subscribers[issi] = {
                "issi": issi,
                "registered_at": time.time(),
                "last_seen": time.time(),
                "groups": groups or [],
            }
            if groups:
                self.affiliations[issi] = groups

    async def remove_subscriber(self, issi: int):
        """Deregistriert einen Teilnehmer."""
        async with self._lock:
            if issi in self.subscribers:
                del self.subscribers[issi]
            if issi in self.affiliations:
                del self.affiliations[issi]

    async def affiliate(self, issi: int, groups: list):
        """Affiliiert einen Teilnehmer mit einer Gruppe."""
        async with self._lock:
            self.affiliations[issi] = groups
            if issi in self.subscribers:
                self.subscribers[issi]["groups"] = groups
                self.subscribers[issi]["last_seen"] = time.time()

    async def deaffiliate(self, issi: int):
        """Deaffiliiert einen Teilnehmer."""
        async with self._lock:
            if issi in self.affiliations:
                del self.affiliations[issi]
            if issi in self.subscribers:
                self.subscribers[issi]["groups"] = []

    def get_all_subscribers(self):
        """Gibt alle registrierten Teilnehmer zurück."""
        return list(self.subscribers.values())

    def get_stats(self):
        """Gibt Statistiken zurück."""
        return {
            "uptime": int(time.time() - self.stats["start_time"]),
            "subscribers": len(self.subscribers),
            "affiliations": len(self.affiliations),
            "active_calls": len(self.active_calls),
            "total_registrations": self.stats["total_registrations"],
            "total_calls": self.stats["total_calls"],
        }

# Globaler Zustand
brew_state = BrewState()

# ============================================================
# HTTP DIGEST AUTHENTICATION (RFC 2831)
# ============================================================
def compute_digest_response(username, realm, password, method, uri, nonce, nc, cnonce, qop):
    """Berechnet die Digest-Response nach RFC 2831."""
    ha1 = hashlib.md5(f"{username}:{realm}:{password}".encode()).hexdigest()
    ha2 = hashlib.md5(f"{method}:{uri}".encode()).hexdigest()
    response = hashlib.md5(
        f"{ha1}:{nonce}:{nc}:{cnonce}:{qop}:{ha2}".encode()
    ).hexdigest()
    return response

def parse_auth_header(auth_header):
    """Parst den Authorization-Header."""
    if not auth_header or not auth_header.startswith("Digest "):
        return None
    parts = auth_header[7:].split(", ")
    params = {}
    for part in parts:
        if "=" in part:
            key, val = part.split("=", 1)
            if val.startswith('"') and val.endswith('"'):
                val = val[1:-1]
            params[key] = val
    return params

def verify_digest_auth(auth_header, method, uri):
    """Verifiziert die Digest-Authentifizierung."""
    params = parse_auth_header(auth_header)
    if not params:
        return False

    username = params.get("username")
    realm = params.get("realm")
    nonce = params.get("nonce")
    uri_param = params.get("uri")
    response = params.get("response")
    nc = params.get("nc")
    cnonce = params.get("cnonce")
    qop = params.get("qop")

    if not all([username, realm, nonce, response]):
        return False

    if username != CONFIG["username"] or realm != CONFIG["realm"]:
        return False

    expected = compute_digest_response(
        username, realm, CONFIG["password"],
        method, uri_param or uri,
        nonce, nc or "00000001", cnonce or "00000000", qop or "auth"
    )

    return response == expected

def generate_nonce():
    """Generiert eine zufällige Nonce."""
    return hashlib.md5(f"{time.time()}:{uuid.uuid4()}".encode()).hexdigest()

# ============================================================
# HTTP SERVER FÜR AUTHENTIFIZIERUNG UND DASHBOARD
# ============================================================
class BrewHTTPHandler(BaseHTTPRequestHandler):
    """HTTP-Handler für Authentifizierung und Web-Dashboard."""

    protocol_version = "HTTP/1.1"

    def log_message(self, format, *args):
        pass  # Silent logging

    def send_auth_challenge(self):
        """Sendet eine 401 Unauthorized mit Digest-Challenge."""
        self.send_response(401)
        self.send_header("WWW-Authenticate", 
            f'Digest realm="{CONFIG["realm"]}", '
            f'nonce="{generate_nonce()}", '
            f'algorithm=MD5, qop="auth"'
        )
        self.send_header("Content-Type", "text/html")
        self.end_headers()
        self.wfile.write(b"<html><body><h1>401 Unauthorized</h1></body></html>")

    def is_authenticated(self):
        """Prüft, ob die Anfrage authentifiziert ist."""
        auth = self.headers.get("Authorization")
        if not auth:
            return False
        method = self.command
        path = self.path
        return verify_digest_auth(auth, method, path)

    def do_GET(self):
        """Verarbeitet GET-Anfragen."""
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path

        # Authentifizierung prüfen
        if not self.is_authenticated():
            self.send_auth_challenge()
            return

        if path == "/" or path == "/dashboard":
            self.serve_dashboard()
        elif path == "/api/status":
            self.serve_api_status()
        elif path == "/api/subscribers":
            self.serve_api_subscribers()
        else:
            self.send_response(404)
            self.end_headers()
            self.wfile.write(b"Not found")

    def serve_dashboard(self):
        """Serviert das Web-Dashboard."""
        stats = brew_state.get_stats()
        subscribers = brew_state.get_all_subscribers()

        html = f"""
        <!DOCTYPE html>
        <html>
        <head>
            <meta charset="utf-8">
            <title>Brew Server Dashboard</title>
            <style>
                body {{ font-family: sans-serif; margin: 20px; background: #f5f5f5; }}
                .container {{ max-width: 1200px; margin: 0 auto; }}
                .card {{ background: white; border-radius: 8px; padding: 20px; margin-bottom: 20px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }}
                .stats {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 15px; }}
                .stat {{ text-align: center; }}
                .stat-value {{ font-size: 28px; font-weight: bold; color: #2196F3; }}
                .stat-label {{ font-size: 14px; color: #666; }}
                table {{ width: 100%; border-collapse: collapse; }}
                th, td {{ padding: 10px; text-align: left; border-bottom: 1px solid #ddd; }}
                th {{ background: #f0f0f0; }}
                .badge {{ display: inline-block; padding: 3px 8px; border-radius: 4px; font-size: 12px; }}
                .badge-online {{ background: #4CAF50; color: white; }}
                .badge-offline {{ background: #f44336; color: white; }}
                .refresh-btn {{ background: #2196F3; color: white; border: none; padding: 10px 20px; border-radius: 4px; cursor: pointer; }}
                .refresh-btn:hover {{ background: #1976D2; }}
                .footer {{ text-align: center; color: #999; font-size: 12px; margin-top: 30px; }}
            </style>
        </head>
        <body>
            <div class="container">
                <h1>🍺 Brew Server Dashboard</h1>
                <p><em>Server: {CONFIG["server_name"]}</em></p>

                <div class="card">
                    <h2>📊 Statistiken</h2>
                    <div class="stats">
                        <div class="stat">
                            <div class="stat-value">{stats["subscribers"]}</div>
                            <div class="stat-label">Registrierte Teilnehmer</div>
                        </div>
                        <div class="stat">
                            <div class="stat-value">{stats["affiliations"]}</div>
                            <div class="stat-label">Affiliierte Gruppen</div>
                        </div>
                        <div class="stat">
                            <div class="stat-value">{stats["active_calls"]}</div>
                            <div class="stat-label">Aktive Anrufe</div>
                        </div>
                        <div class="stat">
                            <div class="stat-value">{stats["total_registrations"]}</div>
                            <div class="stat-label">Registrierungen gesamt</div>
                        </div>
                        <div class="stat">
                            <div class="stat-value">{stats["uptime"] // 3600}h {(stats["uptime"] % 3600) // 60}m</div>
                            <div class="stat-label">Laufzeit</div>
                        </div>
                    </div>
                </div>

                <div class="card">
                    <h2>📡 Teilnehmer</h2>
                    <button class="refresh-btn" onclick="location.reload()">🔄 Aktualisieren</button>
                    <table>
                        <thead>
                            <tr>
                                <th>ISSI</th>
                                <th>Registriert seit</th>
                                <th>Letzte Aktivität</th>
                                <th>Gruppen</th>
                                <th>Status</th>
                            </tr>
                        </thead>
                        <tbody>
        """

        if subscribers:
            for sub in subscribers:
                issi = sub.get("issi", "?")
                registered = datetime.fromtimestamp(sub.get("registered_at", 0)).strftime("%Y-%m-%d %H:%M:%S")
                last_seen = datetime.fromtimestamp(sub.get("last_seen", 0)).strftime("%Y-%m-%d %H:%M:%S")
                groups = ", ".join(str(g) for g in sub.get("groups", [])) or "—"
                status = "🟢 Online" if sub.get("groups") else "🟡 Keine Gruppe"
                html += f"""
                            <tr>
                                <td><strong>{issi}</strong></td>
                                <td>{registered}</td>
                                <td>{last_seen}</td>
                                <td>{groups}</td>
                                <td>{status}</td>
                            </tr>
                """
        else:
            html += """
                            <tr><td colspan="5" style="text-align:center; color:#999;">Keine Teilnehmer registriert</td></tr>
            """

        html += f"""
                        </tbody>
                    </table>
                </div>

                <div class="footer">
                    Brew Server v1.0 · {datetime.now().strftime("%Y-%m-%d %H:%M:%S")}
                </div>
            </div>
        </body>
        </html>
        """

        self.send_response(200)
        self.send_header("Content-Type", "text/html")
        self.send_header("Content-Length", str(len(html)))
        self.end_headers()
        self.wfile.write(html.encode())

    def serve_api_status(self):
        """Serviert den API-Status als JSON."""
        stats = brew_state.get_stats()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(stats).encode())

    def serve_api_subscribers(self):
        """Serviert die Teilnehmerliste als JSON."""
        subs = brew_state.get_all_subscribers()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(subs, default=str).encode())

class ThreadedHTTPServer(ThreadingMixIn, HTTPServer):
    """Thread-fähiger HTTP-Server."""
    daemon_threads = True

def run_http_server():
    """Startet den HTTP-Server in einem separaten Thread."""
    server = ThreadedHTTPServer(("0.0.0.0", CONFIG["http_port"]), BrewHTTPHandler)
    print(f"🌐 HTTP Server läuft auf http://0.0.0.0:{CONFIG['http_port']}")
    print(f"   Login: {CONFIG['username']} / {CONFIG['password']}")
    server.serve_forever()

# ============================================================
# WEBSOCKET BREW SERVER
# ============================================================
async def handle_brew_connection(websocket, path):
    """Verarbeitet eine eingehende Brew-WebSocket-Verbindung."""
    print(f"🔗 Neue Brew-Verbindung: {websocket.remote_address}")
    brew_state.connections.append(websocket)

    try:
        async for message in websocket:
            # Brew-Nachrichten sind binär
            if not isinstance(message, bytes):
                continue

            if len(message) < 2:
                continue

            # Zweibytiges Präfix: Klasse und Typ
            msg_class = message[0]
            msg_type = message[1]
            payload = message[2:]

            print(f"📨 Nachricht: Klasse=0x{msg_class:02x}, Typ=0x{msg_type:02x}, Länge={len(payload)}")

            # Verarbeite Nachricht basierend auf der Klasse
            if msg_class == BREW_SUBSCRIBER_CONTROL:
                await handle_subscriber_control(websocket, msg_type, payload)
            elif msg_class == BREW_CALL_CONTROL:
                await handle_call_control(websocket, msg_type, payload)
            elif msg_class == BREW_FRAME_DATA:
                # Frame-Daten (SDS, etc.) - nur loggen
                print(f"   📦 Frame Data: {payload.hex()[:32]}...")
            elif msg_class == BREW_SERVICE:
                # Service-Nachrichten (JSON)
                try:
                    json_str = payload.decode('utf-8').rstrip('\x00')
                    data = json.loads(json_str)
                    print(f"   📋 Service: {data}")
                except:
                    pass
            else:
                print(f"   ⚠️ Unbekannte Klasse: 0x{msg_class:02x}")

    except websockets.exceptions.ConnectionClosed:
        print(f"🔌 Verbindung geschlossen: {websocket.remote_address}")
    finally:
        if websocket in brew_state.connections:
            brew_state.connections.remove(websocket)

async def handle_subscriber_control(websocket, msg_type, payload):
    """Verarbeitet Subscriber-Control-Nachrichten (Klasse 0xf0)."""
    # Payload-Struktur: uint32_t ISSI, uint64_t time, uint32_t fraction, [uint32_t groups...]
    if len(payload) < 16:  # 4 + 8 + 4 = 16 Bytes
        print("   ⚠️ Payload zu kurz für Subscriber Control")
        return

    issi = struct.unpack("<I", payload[0:4])[0]
    timestamp = struct.unpack("<Q", payload[4:12])[0]
    fraction = struct.unpack("<I", payload[12:16])[0]

    # Extrahiere Gruppen (GSSIs) aus dem Rest
    groups = []
    if len(payload) > 16:
        num_groups = (len(payload) - 16) // 4
        for i in range(num_groups):
            offset = 16 + i * 4
            groups.append(struct.unpack("<I", payload[offset:offset+4])[0])

    if msg_type == BREW_SUBSCRIBER_REGISTER:
        print(f"   📝 REGISTER: ISSI={issi}")
        await brew_state.add_subscriber(issi, groups)

    elif msg_type == BREW_SUBSCRIBER_DEREGISTER:
        print(f"   ❌ DEREGISTER: ISSI={issi}")
        await brew_state.remove_subscriber(issi)

    elif msg_type == BREW_SUBSCRIBER_REREGISTER:
        print(f"   🔄 REREGISTER: ISSI={issi}")
        await brew_state.add_subscriber(issi, groups)

    elif msg_type == BREW_SUBSCRIBER_AFFILIATE:
        print(f"   🔗 AFFILIATE: ISSI={issi} -> Gruppen={groups}")
        await brew_state.affiliate(issi, groups)

    elif msg_type == BREW_SUBSCRIBER_DEAFFILIATE:
        print(f"   🔗 DEAFFILIATE: ISSI={issi}")
        await brew_state.deaffiliate(issi)

    else:
        print(f"   ⚠️ Unbekannter Subscriber-Typ: {msg_type}")

async def handle_call_control(websocket, msg_type, payload):
    """Verarbeitet Call-Control-Nachrichten (Klasse 0xf1)."""
    call_states = {
        CALL_STATE_GROUP_TX: "GROUP_TX",
        CALL_STATE_GROUP_IDLE: "GROUP_IDLE",
        CALL_STATE_SETUP_REQUEST: "SETUP_REQUEST",
        CALL_STATE_SETUP_ACCEPT: "SETUP_ACCEPT",
        CALL_STATE_SETUP_REJECT: "SETUP_REJECT",
        CALL_STATE_CALL_ALERT: "CALL_ALERT",
        CALL_STATE_CONNECT_REQUEST: "CONNECT_REQUEST",
        CALL_STATE_CONNECT_CONFIRM: "CONNECT_CONFIRM",
        CALL_STATE_CALL_RELEASE: "CALL_RELEASE",
    }
    state_name = call_states.get(msg_type, f"UNKNOWN(0x{msg_type:02x})")
    print(f"   📞 CALL: {state_name} (Länge={len(payload)})")

    if msg_type == CALL_STATE_SETUP_REQUEST and len(payload) >= 4:
        # Extrahiere Quell- und Ziel-ISSI
        source = struct.unpack("<I", payload[0:4])[0]
        dest = struct.unpack("<I", payload[4:8])[0] if len(payload) >= 8 else 0
        print(f"      📞 Anruf von {source} -> {dest}")
        brew_state.stats["total_calls"] += 1

    elif msg_type == CALL_STATE_CALL_RELEASE:
        brew_state.active_calls = []

# ============================================================
# MAIN
# ============================================================
async def main():
    """Hauptfunktion: Startet HTTP- und WebSocket-Server."""
    # HTTP-Server in separatem Thread starten
    http_thread = threading.Thread(target=run_http_server, daemon=True)
    http_thread.start()

    # WebSocket-Server starten
    print(f"🔌 Brew WebSocket Server läuft auf ws://0.0.0.0:{CONFIG['ws_port']}")
    print(f"   Authentifizierung: {CONFIG['username']} / {CONFIG['password']}")
    print("   Drücke Ctrl+C zum Beenden")
    print("-" * 50)

    async with websockets.serve(
        handle_brew_connection,
        "0.0.0.0",
        CONFIG["ws_port"],
        max_size=10**7,  # 10 MB
    ):
        await asyncio.Future()  # Läuft bis zum Abbruch

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n👋 Server beendet.")
