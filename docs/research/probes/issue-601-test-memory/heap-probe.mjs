// Issue #601: loaded with NODE_OPTIONS="--import <this file>" into every Node
// process of a suite (vitest fork workers inherit it). Samples
// process.memoryUsage() and appends the process's peaks to
// $HEAP_PROBE_DIR/<pid>.json on exit. Worker threads are not sampled.
import { writeFileSync } from "node:fs";

const dir = process.env.HEAP_PROBE_DIR;
if (dir) {
  const peak = { rss: 0, heapTotal: 0, heapUsed: 0, external: 0, arrayBuffers: 0 };
  const sample = () => {
    const m = process.memoryUsage();
    for (const key of Object.keys(peak)) peak[key] = Math.max(peak[key], m[key]);
  };
  setInterval(sample, 100).unref();
  process.on("exit", () => {
    sample();
    const mib = Object.fromEntries(Object.entries(peak).map(([k, v]) => [k, Math.round(v / 1048576)]));
    try {
      writeFileSync(`${dir}/${process.pid}.json`, JSON.stringify({ pid: process.pid, argv: process.argv.slice(1, 3), ...mib }));
    } catch {}
  });
}
