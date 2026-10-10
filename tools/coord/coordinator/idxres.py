# Resolve conflicts that lie entirely inside <!-- index --> ... <!-- /index --> by keeping HEAD; exit 1 if any conflict is outside.
import re,sys
bad=0
for p in sys.argv[1:]:
    s=open(p).read()
    a=s.find('<!-- index -->'); b=s.find('<!-- /index -->')
    for m in re.finditer(r'<<<<<<< HEAD\n.*?>>>>>>> [^\n]*\n',s,flags=re.S):
        if not (a!=-1 and a<m.start() and m.end()<=b): bad=1; print('OUTSIDE',p)
    if not bad:
        s=re.sub(r'<<<<<<< HEAD\n(.*?)=======\n.*?>>>>>>> [^\n]*\n',r'\1',s,flags=re.S); open(p,'w').write(s)
sys.exit(bad)
