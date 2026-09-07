import assert from 'node:assert/strict';
import test from 'node:test';
import * as core from '@open-sheet/core';
import { translateExpression } from './export.mjs';

test('translates the bounded semantic tree through public open-sheet builders', () => {
  const locations = new Map([
    ['e-left.f-value', { block: 'left', field: 'f-value', row: 0 }],
    ['e-right.f-value', { block: 'right', field: 'f-value', row: 1 }],
  ]);
  const expression = {
    op: 'maximum',
    args: {
      left: { op: 'multiply', args: { left: { op: 'number', args: 4 }, right: { op: 'number', args: 2 } } },
      right: { op: 'reference', args: { entity: 'e-right', field: 'f-value' } },
    },
  };
  const translated = translateExpression(expression, locations, core);
  assert.deepEqual(translated, {
    k: 'fn',
    name: 'MAX',
    args: [
      { k: 'op', op: '*', l: { k: 'lit', v: 4 }, r: { k: 'lit', v: 2 } },
      { k: 'ref', target: { kind: 'cell', block: 'right', part: 'data', column: 'f-value', row: 1 } },
    ],
  });
});

test('rejects an unprojected stable reference instead of inventing an address', () => {
  assert.throws(
    () => translateExpression(
      { op: 'reference', args: { entity: 'missing', field: 'f-value' } },
      new Map(),
      core,
    ),
    (error) => error.code === 'unprojected_reference',
  );
});
