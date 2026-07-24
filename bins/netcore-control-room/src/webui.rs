pub fn index_html(node_path: &str, ui_path: &str) -> String {
    format!(
        r####"<!doctype html>
<html lang="de">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>NetCore Control Room</title>
  <style>
    :root {{ color-scheme: dark; --bg:#071019; --panel:#0d1b28; --panel2:#122437; --line:#23415a; --text:#e7f2fa; --muted:#8eacc0; --ok:#45d483; --warn:#ffc857; --bad:#ff6b6b; --info:#62b6ff; }}
    * {{ box-sizing:border-box; }}
    body {{ margin:0; font-family:Inter,ui-sans-serif,system-ui,-apple-system,"Segoe UI",sans-serif; background:linear-gradient(160deg,#06111b,#091925 55%,#071019); color:var(--text); min-height:100vh; }}
    header {{ position:sticky; top:0; z-index:5; background:rgba(7,16,25,.94); backdrop-filter:blur(12px); border-bottom:1px solid var(--line); padding:16px 24px; display:flex; align-items:center; gap:18px; flex-wrap:wrap; }}
    .brand {{ font-size:20px; font-weight:800; letter-spacing:.02em; }}
    .sub {{ color:var(--muted); font-size:13px; }}
    .warning {{ background:#3b2d0a; color:#ffe4a1; border:1px solid #745817; padding:10px 14px; border-radius:10px; font-weight:700; flex:1; min-width:320px; }}
    .actions {{ display:flex; gap:8px; flex-wrap:wrap; }}
    button,a.button {{ border:1px solid var(--line); background:var(--panel2); color:var(--text); padding:9px 12px; border-radius:9px; cursor:pointer; text-decoration:none; font-weight:700; }}
    button:hover,a.button:hover {{ border-color:var(--info); }}
    main {{ padding:22px; max-width:1700px; margin:auto; }}
    .grid {{ display:grid; gap:14px; }}
    .kpis {{ grid-template-columns:repeat(auto-fit,minmax(150px,1fr)); margin-bottom:18px; }}
    .services {{ grid-template-columns:repeat(auto-fit,minmax(240px,1fr)); }}
    .domains {{ grid-template-columns:repeat(auto-fit,minmax(230px,1fr)); }}
    .metric-list {{ display:grid; grid-template-columns:1fr auto; gap:6px 12px; margin-top:12px; font-size:12px; }}
    .metric-list span:nth-child(odd) {{ color:var(--muted); overflow-wrap:anywhere; }}
    .metric-list span:nth-child(even) {{ font-family:ui-monospace,SFMono-Regular,Consolas,monospace; font-weight:800; text-align:right; }}
    .two {{ grid-template-columns:repeat(auto-fit,minmax(420px,1fr)); margin-top:18px; }}
    .card {{ background:rgba(13,27,40,.94); border:1px solid var(--line); border-radius:14px; padding:16px; box-shadow:0 10px 30px rgba(0,0,0,.18); }}
    .kpi .value {{ font-size:30px; font-weight:850; }}
    .kpi .label {{ color:var(--muted); font-size:12px; text-transform:uppercase; letter-spacing:.08em; }}
    h2 {{ margin:0 0 12px; font-size:17px; }}
    h3 {{ margin:0; font-size:15px; }}
    .service-head {{ display:flex; justify-content:space-between; gap:12px; align-items:flex-start; }}
    .pill {{ display:inline-flex; align-items:center; gap:6px; border-radius:999px; padding:4px 9px; font-size:12px; font-weight:800; border:1px solid currentColor; }}
    .healthy {{ color:var(--ok); }} .degraded,.unknown {{ color:var(--warn); }} .offline {{ color:var(--bad); }} .disabled {{ color:var(--muted); }}
    .service-meta {{ color:var(--muted); font-size:12px; margin-top:10px; line-height:1.55; word-break:break-word; }}
    .service-actions {{ display:flex; gap:8px; margin-top:12px; }}
    table {{ width:100%; border-collapse:collapse; font-size:13px; }}
    th,td {{ text-align:left; padding:9px 8px; border-bottom:1px solid var(--line); vertical-align:top; }}
    th {{ color:var(--muted); font-size:11px; text-transform:uppercase; letter-spacing:.06em; }}
    tbody tr:hover {{ background:rgba(98,182,255,.05); }}
    .scroll {{ max-height:420px; overflow:auto; }}
    form {{ display:grid; gap:9px; margin-top:12px; }}
    input,textarea,select {{ width:100%; background:#081522; border:1px solid var(--line); color:var(--text); border-radius:8px; padding:9px; font:inherit; }}
    textarea {{ min-height:74px; resize:vertical; }}
    .row {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(130px,1fr)); gap:8px; }}
    .muted {{ color:var(--muted); }}
    .critical {{ color:var(--bad); font-weight:800; }}
    .warning-text {{ color:var(--warn); font-weight:700; }}
    .empty {{ color:var(--muted); padding:18px 8px; text-align:center; }}
    details {{ margin-top:12px; }}
    summary {{ cursor:pointer; color:var(--muted); }}
    code {{ font-family:ui-monospace,SFMono-Regular,Consolas,monospace; background:#07131e; padding:2px 5px; border-radius:5px; }}
    footer {{ color:var(--muted); font-size:12px; padding:24px; text-align:center; }}
    @media (max-width:700px) {{ main {{ padding:12px; }} header {{ padding:12px; }} .two {{ grid-template-columns:1fr; }} .warning {{ min-width:0; }} }}
  </style>
</head>
<body>
<header>
  <div><div class="brand">NetCore Control Room</div><div class="sub">Leitstellen- und Bedienebene · Core-Dienste bleiben autoritativ</div></div>
  <div class="warning">OPEN LAB — keine Anmeldung, keine Tokens und kein TLS. Ausschließlich im isolierten Testnetz betreiben.</div>
  <div class="actions">
    <button id="poll">Dienste prüfen</button>
    <a class="button" href="/api/v1/export" target="_blank">Export</a>
    <a class="button" href="/api/v1/openapi.json" target="_blank">API</a>
  </div>
</header>
<main>
  <section class="grid kpis" id="kpis"></section>
  <section class="card">
    <div style="display:flex;justify-content:space-between;gap:12px;align-items:center"><h2>Core- und Edge-Dienste</h2><span class="muted" id="poll-time">noch nicht geprüft</span></div>
    <div class="grid services" id="services"></div>
  </section>

  <section class="card" style="margin-top:18px">
    <div style="display:flex;justify-content:space-between;gap:12px;align-items:center"><h2>Federiertes Kernlagebild</h2><span class="muted">gelesen aus den autoritativen Core-Diensten</span></div>
    <div class="grid domains" id="domains"></div>
  </section>

  <section class="grid two">
    <div class="card">
      <h2>Aktive Notfälle und Rufe</h2>
      <div class="scroll"><table><thead><tr><th>Typ</th><th>Teilnehmer/Ziel</th><th>Node</th><th>Status</th></tr></thead><tbody id="live-operations"></tbody></table></div>
    </div>
    <div class="card">
      <h2>TBS / Edge-Lage</h2>
      <div class="scroll"><table><thead><tr><th>Node</th><th>Standort</th><th>Verbindung</th><th>Teilnehmer</th><th>Rufe</th></tr></thead><tbody id="nodes"></tbody></table></div>
    </div>
    <div class="card">
      <h2>Operator-Schnellaktion</h2>
      <p class="muted">Typisierte Kommandos direkt an eine TBS. Kein generischer Backend-Schreibproxy.</p>
      <form id="command-form">
        <div class="row"><input name="operator_id" placeholder="Operator" value="jan"><input name="node_id" list="node-options" placeholder="Node-ID" required><datalist id="node-options"></datalist></div>
        <div class="row"><select name="action"><option value="kick">Teilnehmer abmelden</option><option value="clear-emergency">Notfall löschen</option><option value="dgna-attach">DGNA Attach</option><option value="dgna-detach">DGNA Detach</option></select><input name="issi" type="number" min="0" max="16777215" placeholder="ISSI" required><input name="gssi" type="number" min="1" max="16777215" placeholder="GSSI nur DGNA"></div>
        <button type="submit">Kommando senden</button>
        <div class="muted" id="command-result">Noch kein Kommando gesendet</div>
      </form>
    </div>
  </section>

  <section class="grid two">
    <div class="card">
      <h2>Einsatz- und Störungsjournal</h2>
      <div class="scroll"><table><thead><tr><th>Schwere</th><th>Titel</th><th>Status</th><th>Zeit</th><th>Aktion</th></tr></thead><tbody id="incidents"></tbody></table></div>
      <details><summary>Manuellen Eintrag anlegen</summary>
        <form id="incident-form">
          <div class="row"><input name="operator_id" placeholder="Operator" value="jan"><select name="severity"><option>warning</option><option>info</option><option>critical</option></select><input name="service" placeholder="Service optional"></div>
          <input name="title" placeholder="Titel" required>
          <textarea name="description" placeholder="Beschreibung"></textarea>
          <button type="submit">Incident anlegen</button>
        </form>
      </details>
    </div>
    <div class="card">
      <h2>Schichtbuch</h2>
      <div class="scroll"><table><thead><tr><th>Zeit</th><th>Operator</th><th>Kategorie</th><th>Eintrag</th></tr></thead><tbody id="shift-log"></tbody></table></div>
      <form id="shift-form">
        <div class="row"><input name="operator_id" placeholder="Operator" value="jan"><input name="category" placeholder="Kategorie" value="general"></div>
        <textarea name="text" placeholder="Schichtbucheintrag" required></textarea>
        <button type="submit">Eintrag speichern</button>
      </form>
    </div>
  </section>

  <section class="card" style="margin-top:18px">
    <h2>Architekturgrenze</h2>
    <p class="muted">Der Control Room liest Lagebilder und stellt Bedienfunktionen bereit. Teilnehmer-, Gruppen-, Mobility-, Call-, SDS-, Packet- und Schlüsselzustände bleiben Eigentum der jeweiligen Core-Dienste. Schreibzugriffe werden nicht als beliebiger HTTP-Proxy durchgereicht; jede Fach-WebUI bleibt direkt erreichbar.</p>
    <p class="muted">Kompatibilitäts-WebSockets: TBS <code>{node_path}</code> · Operatorfeed <code>{ui_path}</code></p>
  </section>
</main>
<footer>NetCore-Tetra Systems · digital. dezentral. skalierbar.</footer>
<script>
const $ = (id) => document.getElementById(id);
const esc = (value) => String(value ?? '').replace(/[&<>"']/g, c => ({{'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#039;'}}[c]));
const fmt = (value) => value ? new Date(value).toLocaleString() : '—';
async function api(path, options={{}}) {{
  const response = await fetch(path, {{headers:{{'Content-Type':'application/json'}}, ...options}});
  const text = await response.text();
  let body = {{}}; try {{ body = text ? JSON.parse(text) : {{}}; }} catch {{ body = {{raw:text}}; }}
  if (!response.ok) throw new Error(body.error || body.message || `HTTP ${{response.status}}`);
  return body;
}}
function renderKpis(data) {{
  const o = data.operations || {{}}; const l = data.legacy || {{}};
  const f = ((data.federated || {{}}).preferred_counts) || {{}};
  const pick = (key, fallback=0) => f[key] ?? fallback ?? 0;
  const rows = [
    ['Dienste gesund', `${{o.services_healthy||0}} / ${{o.services_total||0}}`],
    ['Kritisch offline', o.critical_services_offline||0],
    ['Offene Incidents', (o.incidents_open||0)+(o.incidents_acknowledged||0)],
    ['TBS verbunden', pick('connected_nodes', l.nodes_connected)],
    ['Teilnehmer registriert', pick('subscribers_registered', l.subscribers_online)],
    ['Aktive Rufe', pick('active_calls', l.active_calls_total)],
    ['SDS wartend', pick('sds_queued')],
    ['PDP bereit', pick('packet_contexts_ready')],
    ['Security-Alarme', pick('security_alarms')],
    ['App-Zustellungen', pick('application_deliveries_pending')],
    ['App-Dead-Letter', pick('application_dead_letters')],
    ['Aktive Notfälle', l.emergencies_active||0],
  ];
  $('kpis').innerHTML = rows.map(([label,value]) => `<div class="card kpi"><div class="value">${{esc(value)}}</div><div class="label">${{esc(label)}}</div></div>`).join('');
}}
function renderServices(services) {{
  $('services').innerHTML = services.map(s => `<article class="card">
    <div class="service-head"><div><h3>${{esc(s.display_name)}}</h3><div class="muted">${{esc(s.kind)}}${{s.critical?' · kritisch':''}}</div></div><span class="pill ${{esc(s.status)}}">${{esc(s.status)}}</span></div>
    <div class="service-meta">${{esc(s.base_url)}}<br>live: ${{esc(s.live)}} · ready: ${{esc(s.ready)}} · ${{esc(s.latency_ms ?? '—')}} ms<br>${{esc(s.message || '')}}</div>
    <div class="service-actions"><a class="button" href="${{esc(s.webui_url)}}" target="_blank">WebUI</a><button onclick="toggleService('${{esc(s.name)}}',${{!s.enabled}})">${{s.enabled?'Monitoring aus':'Monitoring an'}}</button></div>
  </article>`).join('') || '<div class="empty">Keine Dienste konfiguriert</div>';
}}
const domainLabels = {{
  'node-gateway':'Node Gateway','subscriber-core':'Teilnehmer','group-core':'Gruppen',
  'mobility-core':'Mobility','call-control':'Call Control','media-switch':'Media',
  'recorder':'Recorder','sds-router':'SDS','packet-core':'Packet Core',
  'ip-gateway':'IP Gateway','security-core':'Security','kmf':'KMF','transit':'Transit',
  'application-gateway':'Applications'
}};
function metricLabel(value) {{ return String(value).replaceAll('_',' '); }}
function renderDomains(federated) {{
  const domains = Object.entries((federated || {{}}).domains || {{}});
  $('domains').innerHTML = domains.map(([name, domain]) => {{
    const metrics = Object.entries(domain.metrics || {{}});
    const metricHtml = metrics.map(([key,value]) => `<span>${{esc(metricLabel(key))}}</span><span>${{esc(value)}}</span>`).join('');
    return `<article class="card"><div class="service-head"><div><h3>${{esc(domainLabels[name]||domain.display_name||name)}}</h3><div class="muted">${{esc(name)}}</div></div><span class="pill ${{esc(domain.status)}}">${{esc(domain.status)}}</span></div><div class="metric-list">${{metricHtml||'<span>noch keine Summary</span><span>—</span>'}}</div></article>`;
  }}).join('') || '<div class="empty">Noch kein federiertes Kernlagebild verfügbar</div>';
}}

async function toggleService(name, enabled) {{
  try {{ await api(`/api/v1/services/${{encodeURIComponent(name)}}/${{enabled?'enable':'disable'}}`, {{method:'POST',body:JSON.stringify({{operator_id:'jan'}})}}); await refresh(); }} catch(error) {{ alert(error.message); }}
}}
function incidentClass(severity) {{ return severity === 'critical' ? 'critical' : severity === 'warning' ? 'warning-text' : ''; }}
function renderIncidents(rows) {{
  $('incidents').innerHTML = rows.map(i => `<tr><td class="${{incidentClass(i.severity)}}">${{esc(i.severity)}}</td><td><strong>${{esc(i.title)}}</strong><br><span class="muted">${{esc(i.description)}}</span></td><td>${{esc(i.status)}}</td><td>${{fmt(i.created_at)}}</td><td>${{i.status==='resolved'?'—':`<button onclick="incidentAction('${{esc(i.id)}}','ack')">Ack</button> <button onclick="incidentAction('${{esc(i.id)}}','resolve')">Lösen</button>`}}</td></tr>`).join('') || '<tr><td colspan="5" class="empty">Keine offenen Störungen 🎉</td></tr>';
}}
async function incidentAction(id, action) {{
  const note = prompt(action === 'ack' ? 'Notiz zur Übernahme (optional)' : 'Lösungsnotiz (optional)') || '';
  try {{ await api(`/api/v1/incidents/${{encodeURIComponent(id)}}/${{action}}`, {{method:'POST',body:JSON.stringify({{operator_id:'jan',note}})}}); await refresh(); }} catch(error) {{ alert(error.message); }}
}}
function renderShift(rows) {{
  $('shift-log').innerHTML = rows.map(i => `<tr><td>${{fmt(i.timestamp)}}</td><td>${{esc(i.operator_id)}}</td><td>${{esc(i.category)}}</td><td>${{esc(i.text)}}</td></tr>`).join('') || '<tr><td colspan="4" class="empty">Noch kein Schichtbucheintrag</td></tr>';
}}
function renderNodes(rows) {{
  $('nodes').innerHTML = rows.map(n => `<tr><td><strong>${{esc(n.station_name||n.node_id)}}</strong><br><span class="muted">${{esc(n.node_id)}}</span></td><td>${{esc(n.site||'—')}}</td><td class="${{n.connected?'healthy':'offline'}}">${{n.connected?'online':'offline'}}</td><td>${{esc(n.subscribers_online)}} / ${{esc(n.subscribers_total)}}</td><td>${{esc(n.active_calls_total)}}</td></tr>`).join('') || '<tr><td colspan="5" class="empty">Keine TBS verbunden</td></tr>';
  $('node-options').innerHTML = rows.map(n => `<option value="${{esc(n.node_id)}}">${{esc(n.station_name||n.node_id)}}</option>`).join('');
}}
function renderLive(emergencies, calls) {{
  const rows = [];
  for (const e of emergencies.emergencies || emergencies.items || []) rows.push(`<tr><td class="critical">Notfall</td><td>ISSI ${{esc(e.source_issi)}} → ${{esc(e.dest_ssi)}}</td><td>${{esc(e.node_id||'—')}}</td><td>aktiv</td></tr>`);
  for (const c of calls.calls || calls.items || []) rows.push(`<tr><td>Ruf</td><td>${{esc(c.call_type||c.kind||'Call')}} · ${{esc(c.destination||c.gssi||c.called_ssi||'—')}}</td><td>${{esc(c.node_id||'—')}}</td><td>${{esc(c.state||c.status||'aktiv')}}</td></tr>`);
  $('live-operations').innerHTML = rows.join('') || '<tr><td colspan="4" class="empty">Keine aktiven Notfälle oder Rufe</td></tr>';
}}
async function refresh() {{
  try {{
    const [overview, services, incidents, shift, emergencies, calls] = await Promise.all([
      api('/api/v1/control-room/overview'), api('/api/v1/services'), api('/api/v1/incidents?limit=100'), api('/api/v1/shift-log?limit=100'), api('/api/emergencies?active=true'), api('/api/calls')
    ]);
    renderKpis(overview); renderServices(services.services||services); renderDomains(overview.federated); renderIncidents(incidents.incidents||incidents); renderShift(shift.entries||shift); renderNodes((overview.legacy||{{}}).nodes||[]); renderLive(emergencies,calls);
    $('poll-time').textContent = `Letzte Prüfung: ${{fmt((overview.operations||{{}}).last_poll_finished_at)}}`;
  }} catch(error) {{ $('poll-time').textContent = `Fehler: ${{error.message}}`; }}
}}
$('poll').addEventListener('click', async () => {{ try {{ await api('/api/v1/services/poll',{{method:'POST',body:'{}'}}); setTimeout(refresh,600); }} catch(error) {{ alert(error.message); }} }});
$('command-form').addEventListener('submit', async (event) => {{
  event.preventDefault();
  const form = Object.fromEntries(new FormData(event.target));
  const node = encodeURIComponent(form.node_id);
  let shortcut = form.action; const body = {{operator_id:form.operator_id, issi:Number(form.issi)}};
  if (form.action.startsWith('dgna-')) {{ shortcut='dgna'; body.gssi=Number(form.gssi); body.attach=form.action==='dgna-attach'; if(!body.gssi){{ $('command-result').textContent='Für DGNA ist eine GSSI erforderlich.'; return; }} }}
  try {{ const result=await api(`/api/nodes/${{node}}/commands/${{shortcut}}`,{{method:'POST',body:JSON.stringify(body)}}); $('command-result').textContent=`Angenommen: ${{result.command_id||result.id||'Kommando gesendet'}}`; await refresh(); }}
  catch(error) {{ $('command-result').textContent=`Fehler: ${{error.message}}`; }}
}});
$('incident-form').addEventListener('submit', async (event) => {{ event.preventDefault(); const form=Object.fromEntries(new FormData(event.target)); try {{ await api('/api/v1/incidents',{{method:'POST',body:JSON.stringify(form)}}); event.target.reset(); await refresh(); }} catch(error) {{ alert(error.message); }} }});
$('shift-form').addEventListener('submit', async (event) => {{ event.preventDefault(); const form=Object.fromEntries(new FormData(event.target)); try {{ await api('/api/v1/shift-log',{{method:'POST',body:JSON.stringify(form)}}); event.target.elements.text.value=''; await refresh(); }} catch(error) {{ alert(error.message); }} }});
refresh(); setInterval(refresh, 5000);
</script>
</body>
</html>"####
    )
}
