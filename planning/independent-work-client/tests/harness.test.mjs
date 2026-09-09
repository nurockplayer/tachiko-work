// Green here validates the oracles and negative controls, NOT the product.
import test from 'node:test';
import assert from 'node:assert/strict';
import * as oracle from './oracles.mjs';
const clone=structuredClone;
const v={occurrence:'session-b',revision:'r2',entity:'stable-e',currentness:'current',impact:3,priority:8};
const samples={
 linked:{occurrence:'session-b',revision:'r2',baseRevision:'r1',entity:'stable-e',sheet:v,brief:clone(v),notes:'先完成試玩回饋，再決定下一版範圍。',sourceHashBefore:'source',sourceHashAfter:'source'},
 rejected:{before:{revision:'r1',canonicalHash:'a'},after:{revision:'r1',canonicalHash:'a'},draftRetained:true},
 saveFailed:{semanticRevision:'r2',publishedRevision:'r2',saved:false,dirty:true,destinationHashBefore:'a',destinationHashAfter:'a'},
 reopened:{before:{occurrence:'session-a',entity:'stable-e'},after:{occurrence:'session-b',entity:'stable-e',impact:3,priority:8,notes:'先完成試玩回饋，再決定下一版範圍。'},browserProcessRestarted:true,authoritativeRead:true},
 unknown:{outcome:'unknown',executeRequests:1,saved:false,staleValuesPresentedAsCurrent:false},
 proposal:{initialHash:'a',beforeApprovalHash:'a',unapprovedHash:'a',approvedPublishCount:1,replayPublishCount:0,stalePublishCount:0,externalEffects:0},
};
for (const [name,sample] of Object.entries(samples)) test(`oracle control: ${name}`,()=>oracle[name](clone(sample)));
const mutants=[
 ['linked','cached calculation',o=>o.brief.priority=10],
 ['linked','old revision',o=>o.brief.revision='r1'],
 ['linked','old occurrence',o=>o.sheet.occurrence='session-a'],
 ['linked','retargeted view',o=>o.brief.entity='other-e'],
 ['linked','source overwrite',o=>o.sourceHashAfter='changed'],
 ['linked','unpublished result',o=>o.baseRevision='r2'],
 ['linked','lost notes',o=>o.notes=''],
 ['rejected','partial mutation',o=>o.after.canonicalHash='b'],
 ['rejected','revision changed',o=>o.after.revision='r2'],
 ['rejected','draft discarded',o=>o.draftRetained=false],
 ['saveFailed','false saved',o=>o.saved=true],
 ['saveFailed','dirty erased',o=>o.dirty=false],
 ['saveFailed','destination replaced',o=>o.destinationHashAfter='b'],
 ['reopened','reused authority',o=>o.after.occurrence='session-a'],
 ['reopened','memory-only save',o=>o.browserProcessRestarted=false],
 ['reopened','cached reopen',o=>o.authoritativeRead=false],
 ['unknown','blind retry',o=>o.executeRequests=2],
 ['unknown','false failure',o=>o.outcome='failed'],
 ['unknown','stale presented current',o=>o.staleValuesPresentedAsCurrent=true],
 ['proposal','preview published',o=>o.beforeApprovalHash='b'],
 ['proposal','unapproved published',o=>o.unapprovedHash='b'],
 ['proposal','replay published',o=>o.replayPublishCount=1],
 ['proposal','stale published',o=>o.stalePublishCount=1],
 ['proposal','host effect leaked',o=>o.externalEffects=1],
];
for(const [name,label,mutate] of mutants) test(`negative control: ${label}`,()=>{const o=clone(samples[name]); mutate(o); assert.throws(()=>oracle[name](o),assert.AssertionError);});
