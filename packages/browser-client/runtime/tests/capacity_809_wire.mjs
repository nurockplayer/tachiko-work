// Scratch #809. Real wasm32 ABI; input fixtures are written by the Rust bench.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { performance } from 'node:perf_hooks';
const { instance: { exports: wasm } } = await WebAssembly.instantiate(await readFile(process.argv[2]), {});
const encoder = new TextEncoder();
const decoder = new TextDecoder();
function put(arena, bytes) {
  const pointer = wasm[`tachiko_designer_${arena}_reserve`](bytes.length);
  new Uint8Array(wasm.memory.buffer, pointer, bytes.length).set(bytes);
}
function take(arena) {
  return new Uint8Array(wasm.memory.buffer, wasm[`tachiko_designer_${arena}_ptr`](),
    wasm[`tachiko_designer_${arena}_len`]()).slice();
}
function reply() {
  const result = JSON.parse(decoder.decode(take('response')));
  assert.equal(result.status, 'ok', JSON.stringify(result));
  return result.response.payload;
}
function request(input) {
  put('request', encoder.encode(JSON.stringify(input)));
  wasm.tachiko_designer_request_run();
  return reply();
}
function open(bytes) {
  put('project', bytes);
  put('request', encoder.encode('00000000-0000-4000-8000-000000000819'));
  wasm.tachiko_designer_project_open_working_size();
  return reply();
}
function measure(action) {
  const started = performance.now();
  const value = action();
  return [value, performance.now() - started];
}
function window(revision) {
  return request({ type: 'query_table_window', expected_revision: revision,
    collection: 'grid', view: {}, start: 0, count: 0, display: true });
}
function percentile(values, p) {
  return [...values].sort((a,b) => a-b)[Math.ceil(values.length*p)-1];
}
const summaries = new Map();
function report(rows, step, values) {
  const median = percentile(values, .5);
  summaries.set(`${rows}:${step}`, median);
  console.log(`| ${rows} | ${step} | ${values.length} | ${median.toFixed(3)} | ${percentile(values,.95).toFixed(3)} | ${Math.max(...values).toFixed(3)} |`);
}
console.log('\n### WASM real ABI (serialization, arena copies and reply parsing included; no phases)');
console.log('| rows | step | n | p50 ms | p95 ms | max ms |\n|---:|---|---:|---:|---:|---:|');
for (const rows of [5000, 10000]) {
  const bytes = await readFile(`${process.env.RUNNER_TEMP}/809-${rows}.twd`);
  const times = [];
  let opened;
  for (let i=0; i<3; i++) {
    const [value, ms] = measure(() => open(bytes));
    opened = value;
    assert.equal(opened.window.total_rows, rows);
    times.push(ms);
  }
  report(rows, 'open', times);
  let revision = opened.bootstrap.revision;
  const edits = [], windows = [], combined = [];
  for (let i=0; i<30; i++) {
    const row = Math.round(i*(rows-1)/29);
    const [publication, ms] = measure(() => request({ type: 'edit_scalar', expected_revision: revision,
      target: {entity: `r${String(row).padStart(5,'0')}`, field:'column_1'},
      input: {kind:'number', input: String(20001+i)} }));
    revision = publication.resulting_revision;
    const [windowed, windowMs] = measure(() => window(revision));
    assert.equal(windowed.rows.length, 0);
    edits.push(ms); windows.push(windowMs); combined.push(ms + windowMs);
  }
  report(rows, 'edit', edits); report(rows, 'edit zero-row window', windows);
  report(rows, 'edit + window', combined);
  for (const columns of [1,7]) {
    const clears=[], windows=[], combined=[];
    for (let repeat=0; repeat<3; repeat++) {
      revision = open(bytes).bootstrap.revision;
      const edits = Array.from({length:rows}, (_, row) => Array.from({length:columns}, (_,c) => ({
        target:{ entity:`r${String(row).padStart(5,'0')}`, field:`column_${c}` }, input:{kind:'clear'},
      }))).flat();
      const [publication, ms] = measure(() => request({type:'edit_cells',expected_revision:revision,edits}));
      const [windowed, windowMs] = measure(() => window(publication.resulting_revision));
      assert.equal(windowed.rows.length,0);
      clears.push(ms); windows.push(windowMs); combined.push(ms+windowMs);
    }
    report(rows,`clear ${columns}`,clears); report(rows,`clear ${columns} zero-row window`,windows);
    report(rows,`clear ${columns} + window`,combined);
  }
}
console.log('\n### WASM growth 5,000 → 10,000 rows');
for (const [key,ms] of summaries) {
  if (key.startsWith('10000:')) {
    const step = key.slice(6);
    console.log(`| ${step} | ${(ms/summaries.get(`5000:${step}`)).toFixed(3)}× |`);
  }
}
