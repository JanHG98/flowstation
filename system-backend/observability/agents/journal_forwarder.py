#!/usr/bin/env python3
"""Optional open-lab journald forwarder for a service LXC."""
import json, os, subprocess, time, urllib.request
ENDPOINT=os.environ.get("NETCORE_OBSERVABILITY_LOG_ENDPOINT","http://127.0.0.1:8210/api/v1/logs/ingest")
SERVICE=os.environ.get("NETCORE_SERVICE_NAME","unknown-service")
NODE=os.environ.get("NETCORE_NODE_NAME",os.uname().nodename)
BATCH=max(1,int(os.environ.get("NETCORE_LOG_BATCH","25")))
proc=subprocess.Popen(["journalctl","-f","-n","0","-o","json"],stdout=subprocess.PIPE,text=True)
buffer=[]
for line in proc.stdout:
    try:
        item=json.loads(line); message=item.get("MESSAGE","")
        if not message: continue
        priority=int(item.get("PRIORITY",6)); level="error" if priority<=3 else "warn" if priority==4 else "info" if priority<=6 else "debug"
        buffer.append({"timestamp":None,"service":SERVICE,"node":NODE,"level":level,"message":str(message),"correlation_id":None,"trace_id":None,"fields":{"unit":item.get("_SYSTEMD_UNIT"),"pid":item.get("_PID")}})
        if len(buffer)<BATCH: continue
        payload=json.dumps({"records":buffer}).encode(); request=urllib.request.Request(ENDPOINT,data=payload,headers={"Content-Type":"application/json"},method="POST")
        try:
            with urllib.request.urlopen(request,timeout=5): pass
            buffer.clear()
        except Exception as error:
            print(f"journal forward failed: {error}",flush=True); time.sleep(2)
    except Exception as error:
        print(f"journal parse failed: {error}",flush=True)
