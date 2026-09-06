// Real upstream rendering, not a fake export receipt or a hand-written HTML substitute.
// Run separately after the probe's pinned pnpm dependencies and Chromium are installed.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { pathToFileURL } from 'node:url';
import {
  assertProjection, base, buffed, digest, hostileName, ids, native, nativeRows,
  project, scratch,
} from './support.mjs';

async function publishAndInspect(t, source, output, theme = 'plain') {
  const data = project(source, output, theme);
  assertProjection(data, source);
  // Lazy imports keep the deliberate seed failure separate from dependency setup.
  const ops = await import('@open-document/core/ops');
  const { chromium } = await import('playwright');
  let browser;
  try {
    // Do not share a warm renderer across distinct workspace roots.
    await ops.closeRenderSession();
    const ctx = ops.makeContext({ userCwd: output, coreVersion: '0.6.0' });
    const check = await ops.checkLayout(ctx, 'moonfall');
    assert.ok(check.pageCount > 0, 'The real renderer must produce pages');
    assert.equal(check.errors, 0, JSON.stringify(check.findings));
    t.diagnostic(`open-doc layout: ${check.pageCount} pages, ${check.warnings} warnings`);
    const exported = await ops.exportDocument(ctx, 'moonfall', { format: 'html' });
    assert.equal(exported.format, 'html');
    assert.equal(exported.pageCount, check.pageCount);
    assert.equal(exported.files.length, 1, 'The no-external-assets fixture exports one HTML file');
    assert.ok(exported.files[0].endsWith('.html'), 'Expected real static HTML, not a bundle receipt');
    const html = path.resolve(output, exported.files[0]);
    assert.ok(html.startsWith(output + path.sep));
    assert.ok(readFileSync(html).length > 0);
    browser = await chromium.launch({ headless: true });
    const context = await browser.newContext({ javaScriptEnabled: false });
    await context.route(/^https?:\/\//, (route) => route.abort());
    const page = await context.newPage();
    await page.goto(pathToFileURL(html).href);
    const stamp = page.locator('[data-tachiko-source-sha256]');
    assert.equal(await stamp.count(), 1);
    assert.equal(await stamp.getAttribute('data-tachiko-source-sha256'), digest(source));
    const metric = page.locator('[data-tachiko-metric]');
    assert.equal(await metric.count(), 1);
    assert.equal((await metric.innerText()).trim(), String(data.metric.value));
    assert.equal(await page.locator('[data-tachiko-validation="valid"]').count(), 1);
    for (const row of nativeRows(source)) {
      for (const key of ['name', 'damage', 'dps']) {
        const cell = page.locator(`tbody [data-tachiko-entity-id="${row.entityId}"]` +
          `[data-tachiko-field-id="${ids[key]}"]`);
        assert.equal(await cell.count(), 1, `Exactly one bound table cell for ${row.entityId}/${key}`);
        assert.equal((await cell.innerText()).trim(), String(row[key]));
      }
    }
    // Inert JSON/source text must never create script elements in the artifact.
    assert.equal(await page.locator('script').filter({ hasText: 'globalThis.__injected' }).count(), 0);
    return data;
  } finally {
    await browser?.close();
    await ops.closeRenderSession();
  }
}

test('publication: real open-doc layout and static HTML preserve both source snapshots', async (t) => {
  const dir = scratch(t);
  const before = await publishAndInspect(t, base, path.join(dir, 'base'));
  const after = await publishAndInspect(t, buffed, path.join(dir, 'buffed'));
  assert.equal(before.metric.value, 40);
  assert.equal(after.metric.value, 50);
});

test('publication: compact layout does not change semantic values', async (t) => {
  await publishAndInspect(t, base, path.join(scratch(t), 'compact'), 'compact');
});

test('publication: Chinese and markup-like text remain literal in the actual export', async (t) => {
  const dir = scratch(t);
  const source = path.join(dir, 'hostile-text.ro');
  native('set', base, 'iron_sword.name', hostileName, '--output', source);
  await publishAndInspect(t, source, path.join(dir, 'report'));
});
