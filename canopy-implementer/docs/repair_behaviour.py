# Repair behaviour of the Implementer: for every failed attempt that has a retry, compares the retry's submitted files with the
# failed attempt's and counts recoveries (a retry whose gate passed). usage: python3 repair_behaviour.py <batch dir>...
# where a batch dir holds 2026-*/run-*/events.jsonl as written by run_step --out.
import json,glob,sys,collections,re,hashlib
def attempts(path):
    """per (step, attempt): submitted files {path: content}, outcome (failed stage/feedback or passed)"""
    ev=[json.loads(l)['event'] for l in open(path)]
    sub={}; out={}
    for e in ev:
        t=e['type']
        if t=='tool_called' and e.get('tool')=='submit_files' and e.get('outcome')=='ok':
            try: files={f['path']:f['content'] for f in json.loads(e['arguments'])['files']}
            except Exception: continue
            sub[(e['step_id'],int(e['attempt']))]=files
        elif t=='attempt_failed':
            out[(e['step_id'],int(e['attempt']))]=('failed',e.get('stage'),e.get('feedback',''))
        elif t=='step_finished':
            pass
    # passed attempt: last attempt of a step with a submit and no failure
    return sub,out
def classify(fb):
    if 'expected:' in fb and 'but was' in fb: return 'wrong expected value'
    if 'Expected' in fb and 'to be thrown' in fb: return 'expected exception not thrown'
    if 'Unexpected exception type' in fb: return 'wrong exception type'
    if 'cannot find symbol' in fb or 'error:' in fb: return 'compile'
    return 'other'
rows=[]
for batch in sys.argv[1:]:
    for f in glob.glob(batch+'/2026*/run-*/events.jsonl'):
        sub,out=attempts(f)
        for (step,a),(st,stage,fb) in sorted(out.items()):
            nxt=sub.get((step,a+1)); cur=sub.get((step,a))
            if not cur or not nxt: continue
            same_all = cur==nxt
            main=[p for p in cur if '/test/' not in p]; test=[p for p in cur if '/test/' in p]
            same_main = all(cur.get(p)==nxt.get(p) for p in main)
            same_test = all(cur.get(p)==nxt.get(p) for p in test)
            recovered = (step,a+1) not in out
            rows.append((classify(fb),same_all,same_main,same_test,recovered))
c=collections.defaultdict(lambda:[0,0,0,0,0])
for k,sa,sm,st,rec in rows:
    r=c[k]; r[0]+=1; r[1]+=sa; r[2]+=sm; r[3]+=st; r[4]+=rec
print('class | retries | identical answer | same production code | same test | recovered')
tot=[0]*5
for k,r in sorted(c.items(), key=lambda x:-x[1][0]):
    print(k,'|',*r); tot=[a+b for a,b in zip(tot,r)]
print('ALL |',*tot)
print()
print('what the retry changed | retries | recovered')
for name,pred in [('nothing (identical answer)',lambda r:r[1]),('production code only',lambda r:(not r[2]) and r[3]),('test only',lambda r:r[2] and not r[3]),('both',lambda r:(not r[2]) and (not r[3]))]:
    sel=[r for r in rows if pred(r)]
    print(name,'|',len(sel),'|',sum(r[4] for r in sel))
print()
print('by class and change | retries | recovered')
for k in sorted(set(r[0] for r in rows)):
    for name,pred in [('identical',lambda r:r[1]),('changed',lambda r:not r[1])]:
        sel=[r for r in rows if r[0]==k and pred(r)]
        print(k,name,'|',len(sel),'|',sum(r[4] for r in sel))
