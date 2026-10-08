#!/usr/bin/env python3
"""Rebuilds docs/handoff/provisional-index.tsv and provisional-index-rec.tsv
(q-provisional-audit): every PROVISIONAL line of specs/ and crates/, and every
REC entry of docs/HANDOFF.md. settle_kind / data_alone_may_settle are keyword
sorts of the REC text, not verdicts. Run: py tools/provisional_index.py"""
import re,subprocess,collections
import os
R=os.path.dirname(os.path.dirname(os.path.abspath(__file__)))+"/"
H=open(R+"docs/HANDOFF.md").read().split("\n")
# --- REC entries of HANDOFF (heading entries and bullet entries)
rec={}   # id -> dict(tag,title,text,line)
i=0
while i<len(H):
    l=H[i]
    m=re.match(r"##### (REC-\d+) (\[[^\]]*\](?: \[[^\]]*\])*)?\s*(.*)",l)
    if m:
        body=[l];j=i+1
        while j<len(H) and not H[j].startswith("#####") and not H[j].startswith("- **REC-") and not H[j].startswith("## "):
            body.append(H[j]);j+=1
        rec.setdefault(m.group(1),[]).append(dict(tag=(m.group(2) or "").strip("[]"),title=m.group(3)[:90],text=" ".join(body),line=i+1,kind="heading"))
        i=j;continue
    m=re.match(r"- \*\*(REC-\d+)\*\* (.*)",l)
    if m:
        rec.setdefault(m.group(1),[]).append(dict(tag="",title=m.group(2)[:90],text=l,line=i+1,kind="bullet"))
    i+=1
# --- settled-by-spec status from pc1-s8 table rows
S=open(R+"docs/handoff/pc1-s8.md").read().split("\n")
settled={}
for l in S:
    if l.startswith("|") and "REC-" in l:
        c=[x.strip() for x in l.strip("|").split("|")]
        if len(c)<5: continue
        recs=re.findall(r"REC-\d+",c[0]); qs=re.findall(r"q-[a-z0-9-]+",c[-1])
        for r in recs: settled.setdefault(r,set()).update(qs)
def classify(t,tag):
    t2=t.lower()
    sc=collections.Counter()
    if tag.startswith("NO RUN") or "no game" in tag: sc["binary"]+=3
    if re.search(r"user's files|user's tables|game-file run|game files|live data|real install|levels\.txt|lvlwarp|data-tool|\.tbl|inventory\.bin|monstats|skills\.txt|hireling\.txt|table rows|town preset|listfile|archive",t2): sc["data"]+=2
    if re.search(r"capture|trace|recording|record_packets|record_frames|screenshot|pixel|breakpoint|hook|per.tick|0x[0-9a-f]{2} bytes",t2): sc["recording"]+=2
    if re.search(r"unwritten|not specified|no spec|is not written|body of|needed for the spec|no written spec|unspec",t2): sc["binary"]+=1
    if not sc: return "unstated"
    return sc.most_common(1)[0][0]
def data_ok(t,tag):
    t2=t.lower()
    return "yes" if re.search(r"user's files|user's tables|game-file run|game files|live data|real install|levels\.txt|lvlwarp|data-tool|\.tbl|inventory\.bin|monstats|skills\.txt|hireling\.txt|table rows|town preset|listfile|vis/warp|\.d2s|mpq",t2) else "no"
recinfo={}
for k,v in rec.items():
    t=" ".join(x["text"] for x in v)
    tag=v[0]["tag"]
    recinfo[k]=dict(kind=classify(t,tag),data=data_ok(t,tag),tag=tag,title=v[0]["title"],line=v[0]["line"],dup=len(v)>1)
# --- provisional points
out=subprocess.run(["git","grep","-n","PROVISIONAL","--","specs","crates"],cwd=R,capture_output=True,text=True).stdout.splitlines()
files=collections.defaultdict(list)
for l in out:
    f,n,_=l.split(":",2); files[f].append(int(n))
cache={}
def lines(f):
    if f not in cache: cache[f]=open(R+f,encoding="utf-8",errors="replace").read().split("\n")
    return cache[f]
def sysof(f):
    p=f.split("/")
    if p[0]=="specs": return "spec/"+p[1]
    return p[1]+"/"+"/".join(p[3:-1] if len(p)>4 else p[2:-1])
rows=[]
for f,ns in sorted(files.items()):
    L=lines(f)
    for n in ns:
        blk=[]
        for i in range(n-1,min(n+5,len(L))):
            if i>n-1 and L[i].strip()=="": break
            blk.append(L[i].strip())
        t=re.sub(r"\s+"," "," ".join(blk))
        direct=sorted(set(re.findall(r"REC-\d+",t)),key=lambda s:int(s[4:]))
        near=[]
        if not direct:
            ctx=" ".join(L[max(0,n-13):n+12])
            near=sorted(set(re.findall(r"REC-\d+",ctx)),key=lambda s:int(s[4:]))[:2]
        ids=direct or near
        i=t.find("PROVISIONAL"); chosen=t[i:i+240].replace("\t"," ")
        m=re.search(r"settled by\s+(.{0,70})",t,re.I)
        settle=m.group(1).rstrip(" ;.,)") if m else ""
        # kind
        known=[recinfo[r] for r in ids if r in recinfo]
        if known:
            kind=known[0]["kind"]; data=known[0]["data"]
            src="rec"
        else:
            kind=classify(t,""); data=data_ok(t,""); src="line"
        status=[]
        for r in ids:
            if r in settled: status.append("spec-settled "+r+"->"+",".join(sorted(settled[r])))
        rows.append(("|".join(direct) if direct else ("~"+"|".join(near) if near else "-"),f"{f}:{n}",sysof(f),chosen,settle,kind,data,"; ".join(status)))
with open(R+"docs/handoff/provisional-index.tsv","w") as o:
    o.write("rec\tlocation\tsystem\tchosen\tsettled_by\tsettle_kind\tdata_alone_may_settle\tstatus\n")
    for r in rows: o.write("\t".join(r)+"\n")
# REC index (second file section: one row per REC entry)
with open(R+"docs/handoff/provisional-index-rec.tsv","w") as o:
    o.write("rec\thandoff_line\ttag\ttitle\tsettle_kind\tdata_alone_may_settle\tduplicated\tcited_in_specs_crates\tspec_settled_by_pc1_s8\n")
    cites=collections.Counter(r for row in rows for r in re.findall(r"REC-\d+",row[0]))
    for k in sorted(recinfo,key=lambda s:int(s[4:])):
        d=recinfo[k]
        o.write("\t".join([k,str(d["line"]),d["tag"],d["title"].replace("\t"," "),d["kind"],d["data"],"yes" if d["dup"] else "",str(cites.get(k,0)),",".join(sorted(settled.get(k,[])))])+"\n")
print(len(rows),len(recinfo))
print(collections.Counter(r[5] for r in rows)); print(collections.Counter(r[6] for r in rows)); print(sum(1 for r in rows if r[7]))
