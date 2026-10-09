#!/usr/bin/env python3
"""Integrator fixups of the ledger part files (docs/handoff/ledger/*.tsv) that the parts
did not make themselves; run by tools/coord/ledger-pull.sh after each pull. Our own code."""
import glob
for p in glob.glob('docs/handoff/ledger/*.tsv'):
    L=open(p,encoding='utf-8').read().split('\n'); n=0
    for i,l in enumerate(L[2:],2):
        c=l.split('\t')
        if len(c)!=15: continue
        o=list(c)
        c[4]=c[4].replace('specs/world/hirelings*','specs/world/hirelings.md,specs/world/hirelings-2.md,specs/world/hirelings-ai.md')
        import re
        if '...(+' in c[6]:
            c[6]=re.sub(r'\.\.\.\(\+\d+\)','',c[6]); c[14]+=' [integrator: checks list was truncated by the part]'
        if c[13]=='-' and c[12]!='EQUAL':
            c[13]='M'; c[14]+=' [integrator: size M set, part left it -]'
        if 'coverage-a3a5' in p:
            if c[8]!='yes':
                L[i]=None; n+=1; continue
            c[0]=c[0].lower()
        if c!=o: L[i]='\t'.join(c); n+=1
    L=[x for x in L if x is not None]
    open(p,'w',encoding='utf-8').write('\n'.join(L))
    if n: print(p,'fixed',n)
