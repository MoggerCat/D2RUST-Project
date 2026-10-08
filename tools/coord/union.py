# usage: union.py FILE... — resolve every conflict block as HEAD side then theirs side
import sys,re
for p in sys.argv[1:]:
    s=open(p).read()
    s,n=re.subn(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n",lambda m:m.group(1)+m.group(2),s,flags=re.S)
    open(p,'w').write(s); print(p,n)
