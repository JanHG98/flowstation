#!/usr/bin/env python3
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone

def compare(v,op,t): return {">":v>t,">=":v>=t,"<":v<t,"<=":v<=t,"==":v==t,"!=":v!=t}[op]

def silence_matches(s,a,now):
    return s["active"] and s["starts"]<=now<s["ends"] and all(s.get(k) in (None,a.get(k)) for k in ("rule_id","service","target_id","severity")) and all(a["labels"].get(k)==v for k,v in s["labels"].items())

def parse_prom_line(line):
    lhs,value=line.rsplit(None,1); name=lhs.split("{",1)[0]; labels={}
    if "{" in lhs:
        body=lhs.split("{",1)[1].rsplit("}",1)[0]
        for item in body.split(","):
            k,v=item.split("=",1); labels[k]=v.strip('"')
    return name,labels,float(value)

name,labels,value=parse_prom_line('netcore_calls_active{service="call-control",kind="group"} 3')
assert name=="netcore_calls_active" and labels["kind"]=="group" and value==3
assert compare(0,"<",1) and compare(3,">=",3) and not compare(1,"!=",1)
now=datetime.now(timezone.utc)
s={"active":True,"starts":now-timedelta(seconds=1),"ends":now+timedelta(minutes=5),"rule_id":None,"service":"call-control","target_id":None,"severity":"critical","labels":{"environment":"open-lab"}}
a={"rule_id":"target-down","service":"call-control","target_id":"cc","severity":"critical","labels":{"environment":"open-lab"}}
assert silence_matches(s,a,now)
s["ends"]=now-timedelta(seconds=1)
assert not silence_matches(s,a,now)
print("Observability reference model: OK")
