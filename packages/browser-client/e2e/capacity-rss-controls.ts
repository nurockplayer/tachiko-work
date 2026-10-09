// Small deterministic sensor/receipt controls. No Chromium or product execution.
import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { collectRssSample, requireCompleteRss, RssMeasurementIncomplete } from "./capacity-rss.ts";
import type { RssSample } from "./capacity-rss.ts";

const MiB = 1024 * 1024;
let clock = 1000;
const status = (mib: number) => `Name:\tcontrolled\nVmRSS:\t${mib * 1024} kB\n`;
const inventory = [{ pid: 11, type: "browser" }, { pid: 22, type: "renderer" }];
const collect = (renderer: string | Error, probe: "live" | "exited" | "unknown", browserMiB = 32) => collectRssSample({
  listProcesses: async () => structuredClone(inventory),
  readStatus: async pid => {
    if (pid === 11) return status(browserMiB);
    if (renderer instanceof Error) throw renderer;
    return renderer;
  },
  probePid: () => {
    if (probe !== "live") throw Object.assign(new Error("controlled probe"), { code: probe === "exited" ? "ESRCH" : "EPERM" });
  },
  now: () => clock++,
});
const records: { name: string; outcome: "complete" | "measurement_unverified" | "budget_exceeded"; samples: RssSample[] }[] = [];
const baseline = await collect(status(32), "live"); // Complete64MiB pre-page baseline.
requireCompleteRss(baseline);
assert.equal(baseline.rss, 64 * MiB);
const complete = await collect(status(400), "live");
requireCompleteRss(complete);
assert.equal(complete.rss, 432 * MiB);
records.push({ name: "complete-measurement-positive", outcome: "complete", samples: [baseline, complete] });
const high = await collect(status(900), "live");
requireCompleteRss(high);
assert.equal(high.rss, 932 * MiB);
records.push({ name: "complete-over-budget-negative", outcome: "budget_exceeded", samples: [baseline, high] });
for (const [name, input, probe] of [
  ["missing-live-renderer", Object.assign(new Error("controlled ENOENT"), { code: "ENOENT" }), "live"],
  ["unparseable-live-renderer", "Name:\tcontrolled\nVmRSS:\tunknown kB\n", "live"],
  ["missing-vmrss-live-renderer", "Name:\tcontrolled\n", "live"],
  ["unproved-exit-permission-error", Object.assign(new Error("controlled EACCES"), { code: "EACCES" }), "unknown"],
] as const) {
  const sample = await collect(input, probe);
  assert.equal(sample.complete, false);
  assert.equal(sample.rss, null);
  assert.equal(sample.processes[1]!.state, "unverified");
  assert.throws(() => requireCompleteRss(sample), RssMeasurementIncomplete);
  records.push({ name, outcome: "measurement_unverified", samples: [baseline, sample] });
}
const exited = await collect(Object.assign(new Error("controlled ENOENT"), { code: "ENOENT" }), "exited");
requireCompleteRss(exited);
assert.equal(exited.rss, 32 * MiB);
assert.equal(exited.processes[1]!.state, "exited");
assert(!("rssBytes" in exited.processes[1]!));
records.push({ name: "proven-exit-positive", outcome: "complete", samples: [baseline, exited] });
// Reproduce the reviewer's forged32MiB aggregate with an omitted900MiB live
// renderer while retaining the independently enumerated two-PID inventory.
const omitted = structuredClone(high);
omitted.processes = omitted.processes.slice(0, 1);
omitted.rss = 32 * MiB;
records.push({ name: "positive-aggregate-omitted-renderer", outcome: "measurement_unverified", samples: [baseline, omitted] });
const noProof = structuredClone(exited);
delete (noProof.processes[1] as { exitProof?: unknown }).exitProof;
records.push({ name: "exit-without-proof", outcome: "measurement_unverified", samples: [baseline, noProof] });
const oldAggregate = { at: 3000, rss: 32 * MiB } as RssSample;
records.push({ name: "legacy-aggregate-only", outcome: "measurement_unverified", samples: [baseline, oldAggregate] });
assert.equal(records.length, 10);
const output = process.argv[2];
assert(output, "new deterministic control capture path required");
await writeFile(output, JSON.stringify({ sensor_controls_only: true, product_execution: false, records }, null, 2) + "\n", { flag: "wx" });
console.log(JSON.stringify({ sensor_controls_only: true, sampler_controls: 7, receipt_vectors: records.length, performance_qualification: false }));
