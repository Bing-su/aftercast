"""Executable acceptance vectors for independently inspected ACT numeric IL.
This is a reference oracle, not an installed aftercast graph implementation.
"""
import json
from pathlib import Path
vectors=json.loads(Path(__file__).with_name('contracts.json').read_text())
for case in vectors['line']:
    duration,window=case['duration'],case['window']
    raw=[0]*(duration+1 if case['rolling'] else duration//window+2)
    for seconds,amount in case['actions']:
        if amount<=0: continue
        offset=int(seconds)
        if case['rolling']:
            if offset<0: continue
            for index in range(offset,min(offset+window,duration)): raw[index]+=amount
        else: raw[int(offset/window)+1]+=amount
    assert raw==case['raw'],case['name']
    rates=[int(value/window) for value in raw]
    assert rates==case['integerRates'],case['name']
    plotted=duration if case['rolling'] else duration//window
    assert rates[:plotted]==case['plottedRates'],case['name']
d=vectors['distribution']
assert [value/d['allSql'] for value in d['attackSql'] if value>0]==d['includedRatios']
r=vectors['resist']; totals={}
for name,amount in r['actions']:
    if name.startswith(r['wardPrefix']): name=name[r['stripUtf16Units']:]
    totals.setdefault(name,0)
    if amount>0: totals[name]+=amount
assert sum(totals.values())==r['total']
assert {name:value/r['total'] for name,value in totals.items()}==r['ratios']
a=vectors['actionBars']; maximum=max([a['maximumFloor']]+a['comparerOrderedAmounts'])
assert maximum==a['maximum']
assert [value/a['axisCeiling'] for value in a['comparerOrderedAmounts']]==a['relativePixelHeights']
for case in vectors['axisCases']:
    maximum=max(1,case['rawMaximum'])
    ceiling=10**len(str(maximum))
    while ceiling//2>maximum: ceiling//=2
    if ceiling/1.25>maximum: ceiling=round(ceiling/1.25)
    assert ceiling==case['ceiling']
print('Graph numeric acceptance vectors passed; native rendering and production integration untested.')
