import re, csv, json, sys, collections
root = sys.argv[1]
src = open(f'{root}/re/decomp-all/XIP.c').read()
# split into functions
funcs = {}
for m in re.finditer(r'// ==== ([0-9a-f]{8}) (\S+)\n(.*?)(?=\n// ==== |\Z)', src, re.S):
    funcs[m.group(1)] = (m.group(2), m.group(3))
SND = {'10024c60':'play','10024c78':'play_vol','10024e08':'loop','10024e30':'div','10024f80':'div_loop','10023f20':'play_vol','10023f54':'loop','10024f60':'music'}
num = r'(0x[0-9a-fA-F]+|\d+)'
calls = collections.defaultdict(list)   # func -> [(sound, kind)]
edges = collections.defaultdict(set)
for addr,(name,body) in funcs.items():
    for c in re.findall(r'FUN_([0-9a-f]{8})\(', body):
        edges[addr].add(c)
    for k,kind in SND.items():
        for m in re.finditer(r'FUN_'+k+r'\(\s*'+num+r'\b', body):
            calls[addr].append((int(m.group(1),0), kind))
    # timeout helper: FUN_100027a0(frames, sound) plays `sound` when the wait runs out
    for m in re.finditer(r'FUN_100027a0\([^,()]+,\s*'+num+r'\)', body):
        v=int(m.group(1),0)
        if v < 0x57: calls[addr].append((v, 'on timeout'))
# motion script plays with literal script id
motions = json.load(open(f'{root}/data/game.json'))['motions']
def script_sounds(i, seen=None):
    seen = seen or set()
    if i in seen or i >= len(motions): return set()
    seen.add(i); s = motions[i]; out=set(); pc=0
    # walk words: header word then commands (op | flags) followed by params; reuse decode via op scan
    for j,w in enumerate(s):
        pass
    return out
# use decode_motion tool output instead: parse sound / spawn lines
import subprocess
dec = {}
def decode(i):
    if i in dec: return dec[i]
    r = subprocess.run(['python3','-I',f'{root}/docs/spec/tools/decode_motion.py',str(i)],capture_output=True,text=True,cwd=root).stdout
    snds = set(int(x) for x in re.findall(r'sound A=(\d+)', r))
    kids = set(int(x) for x in re.findall(r'spawn_script A=(\d+)', r))
    dec[i] = (snds, kids); return dec[i]
def script_snds(i, seen):
    if i in seen: return set()
    seen.add(i); s,k = decode(i); out=set(s)
    for c in k: out |= script_snds(c, seen)
    return out
PLAY = ['10000028','10000854','100000bc']
for addr,(name,body) in funcs.items():
    for k in PLAY:
        for m in re.finditer(r'FUN_'+k+r'\([^,]+,\s*'+num+r'\b', body):
            sid = int(m.group(1),0)
            if 0 < sid < len(motions):
                for snd in script_snds(sid, set()):
                    calls[addr].append((snd, f'script 0x{sid:x}'))
# scenes
scenes = list(csv.DictReader(open(f'{root}/docs/spec/data/scenes.csv')))
reach = collections.defaultdict(dict)  # func -> scene -> depth
for sc in scenes:
    q = [(sc[k],0) for k in ('init','update','draw','exit')]
    seen = {}
    while q:
        f,d = q.pop(0)
        if f in seen or d > 6: continue
        seen[f] = d
        for c in edges.get(f, ()): q.append((c, d+1))
    for f,d in seen.items():
        if f in calls: reach[f][sc['name']] = d
names = {}
for line in open(f'{root}/docs/spec/data/sounds.csv').readlines()[1:]:
    p=line.split(','); names[int(p[0])] = p[4]
rows=[]
for f, lst in calls.items():
    sc = reach.get(f, {})
    if sc:
        m = min(sc.values()); best = sorted(k for k,v in sc.items() if v==m)
    else: best = []
    for snd, kind in lst:
        rows.append((snd, f, funcs[f][0], kind, ';'.join(best[:6]) + (f' (+{len(best)-6})' if len(best)>6 else '')))
rows.sort()
w = csv.writer(open(sys.argv[2],'w'))
w.writerow(['sound','function','name','how','scenes'])
for r in rows: w.writerow(r)
used = collections.defaultdict(set)
for r in rows: used[r[0]].add(r[4] or r[1])
print(len(rows), 'calls;', len(used), 'distinct sounds of', len(names))
print('never referenced:', sorted(set(names)-set(used)))
