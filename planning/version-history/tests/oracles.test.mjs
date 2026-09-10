// Synthetic positive/negative controls ONLY. Never count these as product tests.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as o from './oracles.mjs';
const copy = x => structuredClone(x);
const files = Object.fromEntries(o.FILES.map(x => [x, x.startsWith('entities/') ? '' : '{}\n']));
const a = {documentId:'synthetic-doc',occurrence:'synthetic-run',revision:'a',impact:5,priority:10,notes:o.NOTE_A,canonicalFiles:files};
const b = {...copy(a),revision:'b',impact:3,priority:8,notes:o.NOTE_B,canonicalFiles:{...files,'entities/2.jsonl':'synthetic changed bytes\n'}};
const restored = {...copy(a),revision:'c'};
const cp = {id:'checkpoint-a',documentId:a.documentId,profile:'snapshot-only',committed:true,canonicalFiles:copy(files)};
const delta = {complete:true,base:{occurrence:b.occurrence,revision:b.revision},changes:[
  {entity:o.ENTITY,field:o.IMPACT,before:{kind:'number',value:5},after:{kind:'number',value:3}},
  {entity:o.ENTITY,field:o.NOTES,before:{kind:'text',value:o.NOTE_A},after:{kind:'text',value:o.NOTE_B}},
]};
const controls = [
  ['snapshot inventory', () => o.snapshot(a), () => {const x=copy(a); delete x.canonicalFiles['entities/0.jsonl']; o.snapshot(x);}],
  ['known runtime values', () => o.values(b,3,8,o.NOTE_B), () => o.values({...b,priority:10},3,8,o.NOTE_B)],
  ['read has no publication', () => o.unchanged(a,copy(a)), () => o.unchanged(a,{...a,revision:'new'})],
  ['canonical equivalence', () => o.equivalent(a,restored), () => o.equivalent(a,{...restored,canonicalFiles:b.canonicalFiles})],
  ['forward publication', () => o.forward(a,b,restored), () => o.forward(a,b,{...restored,revision:'b'})],
  ['no old-occurrence resurrection', () => o.forward(a,b,restored), () => o.forward(a,b,{...restored,occurrence:'old reopened occurrence'})],
  ['durable checkpoint', () => o.checkpoint(cp,a), () => o.checkpoint({...cp,committed:false},a)],
  ['checkpoint scope', () => o.checkpoint(cp,a), () => o.checkpoint({...cp,documentId:'another document'},a)],
  ['exact direct diff', () => o.exactDiff(delta,b), () => {const x=copy(delta); x.changes.pop(); o.exactDiff(x,b);}],
  ['derived value is not an authored edit', () => o.exactDiff(delta,b), () => {const x=copy(delta); x.changes.push({entity:o.ENTITY,field:'priority',before:10,after:8}); o.exactDiff(x,b);}],
  ['preview completeness', () => o.exactDiff(delta,b), () => o.exactDiff({...delta,complete:false},b)],
  ['preview exact base', () => o.exactDiff(delta,b), () => o.exactDiff({...delta,base:{occurrence:b.occurrence,revision:'a'}},b)],
  ['semantic vs durable success', () => o.committed({semantic:'published',persistence:'committed'}), () => o.committed({semantic:'published',persistence:'failed'})],
  ['confirmed saved state', () => o.savedFiles({status:'confirmed',canonicalFiles:a.canonicalFiles},a), () => o.savedFiles({status:'confirmed',canonicalFiles:b.canonicalFiles},a)],
];
for (const [name,good,bad] of controls) {
  test(`oracle positive: ${name}`, good);
  test(`oracle negative: ${name}`, () => assert.throws(bad, assert.AssertionError));
}
