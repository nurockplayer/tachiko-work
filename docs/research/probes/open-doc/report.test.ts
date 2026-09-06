import assert from 'node:assert/strict';
import test from 'node:test';
import { parseArgs, renderTemplate } from './report.ts';

test('unit: command parser rejects duplicate or incomplete flags', () => {
  assert.throws(() => parseArgs(['--source', 'a', '--source', 'b']), { code: 'USAGE' });
  assert.throws(() => parseArgs(['--source', 'a', '--out', 'b', '--tachiko', 'c', '--theme', 'dark']), { code: 'USAGE' });
});
test('unit: fixed templates carry only presentation variation', () => {
  const plain = renderTemplate('plain');
  const compact = renderTemplate('compact');
  for (const template of [plain, compact]) {
    assert.match(template, /import projection from '..\/..\/projection.json'/);
    assert.match(template, /data-tachiko-source-sha256/);
    assert.match(template, /data-tachiko-field-id/);
    assert.match(template, /satisfies DocEntry\[\]/);
    assert.doesNotMatch(template, /globalThis\.__injected/);
  }
  assert.notEqual(plain, compact);
});
