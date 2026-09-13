// Steward-owned observation oracles. These do not implement a semantic engine.
import assert from 'node:assert/strict';
export function linked(o) {
 assert.ok(o.occurrence && o.revision, 'Current runtime identity is required');
 assert.notEqual(o.revision,o.baseRevision,'A successful edit must advance the revision');
 for (const view of [o.sheet,o.brief]) {
  assert.equal(view.occurrence,o.occurrence,'Old occurrence rendered as current');
  assert.equal(view.revision,o.revision,'Mixed or stale revision rendered as current');
  assert.equal(view.entity,o.entity,'View silently retargeted its work item');
  assert.equal(view.currentness,'current');
  assert.equal(view.impact,3);
  assert.equal(view.priority,8);
 }
 assert.equal(o.notes,'先完成試玩回饋，再決定下一版範圍。');
 assert.equal(o.sourceHashAfter,o.sourceHashBefore,'Opening/editing changed the source');
}
export function rejected(o) {
 assert.equal(o.after.revision,o.before.revision,'Rejected command advanced state');
 assert.equal(o.after.canonicalHash,o.before.canonicalHash,'Rejected command partially published');
 assert.equal(o.draftRetained,true,'Rejected input was discarded');
}
export function saveFailed(o) {
 assert.equal(o.semanticRevision,o.publishedRevision,'Failed save rolled back accepted work');
 assert.equal(o.saved,false,'Save failure was reported as saved');
 assert.equal(o.dirty,true,'Save failure erased dirty state');
 assert.equal(o.destinationHashAfter,o.destinationHashBefore,'Failed save replaced destination');
}
export function reopened(o) {
 assert.notEqual(o.after.occurrence,o.before.occurrence,'Reopen reused occurrence authority');
 assert.equal(o.after.entity,o.before.entity,'Durable identity changed across reopening');
 assert.equal(o.after.impact,3);
 assert.equal(o.after.priority,8);
 assert.equal(o.after.notes,'先完成試玩回饋，再決定下一版範圍。');
 assert.equal(o.browserProcessRestarted,true,'Same-process round trip is not restart durability');
 assert.equal(o.authoritativeRead,true,'Rendered cache is not canonical reopen evidence');
}
export function unknown(o) {
 assert.equal(o.outcome,'unknown','Lost result was collapsed to success/failure');
 assert.equal(o.executeRequests,1,'An ambiguous Execute was automatically retried');
 assert.equal(o.saved,false);
 assert.equal(o.staleValuesPresentedAsCurrent,false);
}
export function proposal(o) {
 assert.equal(o.beforeApprovalHash,o.initialHash,'Propose/preview mutated canonical data');
 assert.equal(o.unapprovedHash,o.initialHash,'Unapproved delegated Execute published');
 assert.equal(o.approvedPublishCount,1);
 assert.equal(o.replayPublishCount,0);
 assert.equal(o.stalePublishCount,0);
 assert.equal(o.externalEffects,0,'Semantic approval authorized an unrelated host effect');
}
