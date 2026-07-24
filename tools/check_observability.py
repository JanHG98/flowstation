#!/usr/bin/env python3
from pathlib import Path
import json
import re
import subprocess
import sys
import tempfile
import tomllib

ROOT=Path(__file__).resolve().parents[1]
REQUIRED=[
    "system-backend/observability/Cargo.toml",
    "system-backend/observability/README.md",
    "system-backend/observability/src/main.rs",
    "system-backend/observability/src/config.rs",
    "system-backend/observability/src/collector.rs",
    "system-backend/observability/src/state.rs",
    "system-backend/observability/src/http.rs",
    "system-backend/observability/config/observability.example.toml",
    "system-backend/observability/systemd/netcore-observability.service",
    "system-backend/observability/install/install.sh",
    "system-backend/observability/install/install-stack.sh",
    "system-backend/observability/stack/prometheus/prometheus.yml",
    "system-backend/observability/stack/prometheus/rules/netcore.rules.yml",
    "system-backend/observability/stack/alertmanager/alertmanager.yml",
    "system-backend/observability/stack/loki/loki.yml",
    "system-backend/observability/stack/promtail/promtail.yml",
    "system-backend/observability/stack/grafana/dashboards/netcore-overview.json",
    "system-backend/observability/tests/observability_reference.py",
    "Docs/SWMI_CORE_1_PACKAGE_M_OBSERVABILITY.md",
]
MARKERS={
    "system-backend/observability/src/main.rs":"collector::spawn_collector",
    "system-backend/observability/src/config.rs":"0.0.0.0:8210",
    "system-backend/observability/src/config.rs#2":"token_auth: false",
    "system-backend/observability/src/collector.rs":"parse_prometheus",
    "system-backend/observability/src/state.rs":"netcore_observability_target_up",
    "system-backend/observability/src/state.rs#2":"diagnostic.create",
    "system-backend/observability/src/http.rs":"/api/v1/logs/ingest",
    "system-backend/observability/src/http.rs#2":"OPEN LAB",
    "system-backend/services.toml":"management_port = 8210",
    "Cargo.toml":"system-backend/observability",
}

def rust_balanced(path:Path):
    text=path.read_text(errors="replace"); stack=[]; pairs={')':'(',']':'[','}':'{'}; i=0; line=1
    while i<len(text):
        if text[i]=='\n': line+=1; i+=1; continue
        if text.startswith('//',i):
            end=text.find('\n',i); i=len(text) if end<0 else end; continue
        if text.startswith('/*',i):
            depth=1;i+=2
            while i<len(text) and depth:
                if text.startswith('/*',i):depth+=1;i+=2
                elif text.startswith('*/',i):depth-=1;i+=2
                else:
                    if text[i]=='\n':line+=1
                    i+=1
            if depth:return f"unterminated block comment at line {line}"
            continue
        if text[i]=='r':
            m=re.match(r'r(#{0,255})"',text[i:])
            if m:
                hashes=m.group(1); end_marker='"'+hashes; start=i+2+len(hashes); end=text.find(end_marker,start)
                if end<0:return f"unterminated raw string at line {line}"
                line+=text[i:end+len(end_marker)].count('\n');i=end+len(end_marker);continue
        if text[i]=='"':
            i+=1
            while i<len(text):
                if text[i]=='\\':i+=2;continue
                if text[i]=='"':i+=1;break
                if text[i]=='\n':line+=1
                i+=1
            continue
        if text[i]=="'":
            end=i+1
            while end<min(len(text),i+16):
                if text[end]=="'" and text[end-1]!='\\':i=end+1;break
                end+=1
            else:i+=1
            continue
        c=text[i]
        if c in '([{':stack.append((c,line))
        elif c in ')]}':
            if not stack or stack[-1][0]!=pairs[c]:return f"delimiter mismatch {c} at line {line}"
            stack.pop()
        i+=1
    if stack:return f"unclosed delimiter {stack[-1][0]} from line {stack[-1][1]}"
    return None

def main():
    errors=[]
    for rel in REQUIRED:
        if not (ROOT/rel).is_file():errors.append(f"missing {rel}")
    for synthetic,marker in MARKERS.items():
        rel=synthetic.split('#',1)[0]; p=ROOT/rel
        if not p.is_file() or marker not in p.read_text(errors='replace'):errors.append(f"missing marker {marker!r} in {rel}")
    for rel in ["Cargo.toml","system-backend/services.toml","system-backend/observability/Cargo.toml","system-backend/observability/config/observability.example.toml"]:
        try:tomllib.loads((ROOT/rel).read_text())
        except Exception as e:errors.append(f"invalid TOML {rel}: {e}")
    try:
        c=tomllib.loads((ROOT/'system-backend/observability/config/observability.example.toml').read_text())
        if c['security']['token_auth'] or c['security']['tls'] or c['security']['mode']!='open_lab':errors.append('example must remain open_lab without token/TLS')
        if c['server']['bind'].split(':')[-1]!='8210':errors.append('management port must be 8210')
        if len(c.get('targets',[]))<14:errors.append('example must contain all implemented service targets')
        if len(c.get('alert_rules',[]))<3:errors.append('example must contain baseline alert rules')
    except Exception:pass
    for rel in ["system-backend/observability/src/main.rs","system-backend/observability/src/config.rs","system-backend/observability/src/collector.rs","system-backend/observability/src/protocol.rs","system-backend/observability/src/state.rs","system-backend/observability/src/http.rs"]:
        err=rust_balanced(ROOT/rel)
        if err:errors.append(f"{rel}: {err}")
    try:json.loads((ROOT/'system-backend/observability/stack/grafana/dashboards/netcore-overview.json').read_text())
    except Exception as e:errors.append(f"invalid Grafana JSON: {e}")
    web=(ROOT/'system-backend/observability/src/http.rs').read_text();m=re.search(r'<script>(.*)</script>',web,re.S)
    if not m:errors.append('embedded WebUI JavaScript not found')
    else:
        with tempfile.NamedTemporaryFile('w',suffix='.js',delete=False) as h:h.write(m.group(1));js=Path(h.name)
        try:
            r=subprocess.run(['node','--check',str(js)],capture_output=True,text=True)
            if r.returncode:errors.append(f"WebUI JavaScript invalid: {r.stderr.strip()}")
        except FileNotFoundError:pass
        finally:js.unlink(missing_ok=True)
    for script in sorted((ROOT/'system-backend/observability/install').glob('*.sh')):
        r=subprocess.run(['bash','-n',str(script)],capture_output=True,text=True)
        if r.returncode:errors.append(f"shell syntax {script.relative_to(ROOT)}: {r.stderr.strip()}")
    r=subprocess.run([sys.executable,str(ROOT/'system-backend/observability/tests/observability_reference.py')],capture_output=True,text=True)
    if r.returncode:errors.append(f"reference test failed: {r.stderr.strip() or r.stdout.strip()}")
    if errors:print('\n'.join(errors),file=sys.stderr);return 1
    print('Observability static package check: OK');return 0
if __name__=='__main__':raise SystemExit(main())
