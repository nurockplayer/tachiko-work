// Tests-only upstream canary. Install/build the pinned package in a scratch
// consumer; resolution below obeys its public package exports, not src paths.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const consumer = process.env.OPEN_SHEET_CONSUMER_DIR;
const outputIndex = process.argv.indexOf('--output');
assert(consumer, 'Set OPEN_SHEET_CONSUMER_DIR to the pinned scratch consumer');
assert(outputIndex >= 0 && process.argv[outputIndex + 1], 'Supply --output <absent.xlsx>');
const require = createRequire(resolve(consumer, 'package.json'));
const core = await import(pathToFileURL(require.resolve('@open-sheet/core')).href);
const node = await import(pathToFileURL(require.resolve('@open-sheet/core/node')).href);
const manifest = JSON.parse(await readFile(require.resolve('@open-sheet/core/package.json'), 'utf8'));
assert.equal(typeof core.compile, 'function');
assert.equal(typeof core.ref, 'function');
assert.equal(typeof node.XlsxWriter, 'function');
const book = {
  sheets: [{
    name: 'Probe',
    cells: new Map([
      [core.cellKey(0, 0), { value: 1 }],
      [core.cellKey(1, 0), { expr: core.add(1, 2) }],
    ]),
    columnWidths: new Map(), conditionalFormats: [], charts: [],
    autoFilters: [], sparklines: [], printArea: [], pageBreaks: [],
    bounds: { rows: 2, cols: 1 },
  }],
  registry: new Map(), definedNames: new Map(),
};
const bytes = await new node.XlsxWriter().write(book, { cacheValues: false });
assert(Buffer.isBuffer(bytes));
await writeFile(process.argv[outputIndex + 1], bytes, { flag: 'wx' });
console.log(JSON.stringify({ package: manifest.name, version: manifest.version,
  publicEntries: ['@open-sheet/core', '@open-sheet/core/node'], cacheValues: false }));
