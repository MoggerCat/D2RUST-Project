# usage: renum.py OLD NEW  (run in repo root during a merge) — renumber OLD->NEW only on lines not present in HEAD's version of each file
import sys,subprocess,re
old,new=sys.argv[1],sys.argv[2]
files=subprocess.run(['git','diff','HEAD','--name-only'],capture_output=True,text=True).stdout.split()
pat=re.compile(re.escape(old)+r'(?![0-9A-Za-z-])')
for f in files:
    try: cur=open(f).read().split('\n')
    except FileNotFoundError: continue
    r=subprocess.run(['git','show','HEAD:'+f],capture_output=True,text=True)
    base=set(r.stdout.split('\n')) if r.returncode==0 else set()
    n=0
    for i,l in enumerate(cur):
        if l not in base and pat.search(l): cur[i]=pat.sub(new,l); n+=1
    if n: open(f,'w').write('\n'.join(cur)); print(f,n)
