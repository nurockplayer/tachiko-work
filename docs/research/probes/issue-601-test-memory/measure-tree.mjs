#!/usr/bin/env node
// Issue #601: run one command and sample its whole process tree from /proc
// (Linux only). Prints one JSON line: elapsed time, exit code, peak tree
// PSS/RSS, the per-category split at the tree peak, and each category's own
// peak. PSS counts shared pages once; summed RSS double-counts them.
//
// usage: measure-tree.mjs --label <name> [--interval-ms 200] -- <command...>
import { spawn } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";

const args = process.argv.slice(2);
const sep = args.indexOf("--");
if (sep < 0 || sep === args.length - 1) {
  console.error("usage: measure-tree.mjs --label <name> [--interval-ms N] -- <command...>");
  process.exit(2);
}
const opts = Object.fromEntries(
  args.slice(0, sep).reduce((pairs, value, i, all) => (i % 2 === 0 ? [...pairs, [value.replace(/^--/, ""), all[i + 1]]] : pairs), []),
);
const command = args.slice(sep + 1);
const intervalMs = Number(opts["interval-ms"] ?? 200);
const pageKiB = 4;

function category(comm, cmdline) {
  if (comm === "rustc" || comm === "clippy-driver") return "rustc";
  if (/^(ld|ld\.\w+|cc|gcc|collect2|lld|rust-lld|mold|clang)$/.test(comm)) return "linker";
  if (comm === "cargo") return "cargo";
  if (/^(chrome|chromium|headless_shell|chrome_crashpad)/.test(comm) || /chrom(e|ium)/.test(cmdline)) return "browser";
  if (comm === "node" || comm.startsWith("node ") || /\bnode\b/.test(cmdline.split(" ")[0] ?? "")) return "node";
  if (/\/target[^ ]*\/deps\//.test(cmdline) || /\/target[^ ]*\/(debug|release)\/build\//.test(cmdline)) return "test-or-build-script";
  if (comm === "docker" || comm === "containerd-shim") return "docker-client";
  return "other";
}

function readTree(rootPid) {
  const procs = new Map();
  for (const name of readdirSync("/proc")) {
    if (!/^\d+$/.test(name)) continue;
    try {
      const stat = readFileSync(`/proc/${name}/stat`, "utf8");
      const close = stat.lastIndexOf(")");
      const comm = stat.slice(stat.indexOf("(") + 1, close);
      const ppid = Number(stat.slice(close + 2).split(" ")[1]);
      procs.set(Number(name), { comm, ppid });
    } catch {
      // the process exited between readdir and read
    }
  }
  const children = new Map();
  for (const [pid, p] of procs) {
    if (!children.has(p.ppid)) children.set(p.ppid, []);
    children.get(p.ppid).push(pid);
  }
  const tree = [];
  const stack = [rootPid];
  while (stack.length) {
    const pid = stack.pop();
    if (!procs.has(pid)) continue;
    tree.push(pid);
    stack.push(...(children.get(pid) ?? []));
  }
  return tree.map((pid) => ({ pid, comm: procs.get(pid).comm }));
}

function memoryOf(pid) {
  let pss = 0;
  let rss = 0;
  try {
    for (const line of readFileSync(`/proc/${pid}/smaps_rollup`, "utf8").split("\n")) {
      if (line.startsWith("Pss:")) pss = Number(line.split(/\s+/)[1]);
      if (line.startsWith("Rss:")) rss = Number(line.split(/\s+/)[1]);
    }
  } catch {
    try {
      rss = Number(readFileSync(`/proc/${pid}/statm`, "utf8").split(" ")[1]) * pageKiB;
      pss = rss;
    } catch {
      return null;
    }
  }
  let cmdline = "";
  try {
    cmdline = readFileSync(`/proc/${pid}/cmdline`, "utf8").replaceAll("\0", " ");
  } catch {}
  return { pss, rss, cmdline };
}

const started = process.hrtime.bigint();
const child = spawn(command[0], command.slice(1), { stdio: "inherit" });
const peak = { pssKiB: 0, rssKiB: 0, processes: 0, atSec: 0, byCategoryPssKiB: {} };
const categoryPeakPssKiB = {};
const categoryMaxProcesses = {};
let samples = 0;
const topProcess = { pssKiB: 0, comm: "", cmdline: "" };

function sample() {
  const byCat = {};
  const counts = {};
  let pss = 0;
  let rss = 0;
  const tree = readTree(child.pid);
  for (const { pid, comm } of tree) {
    const m = memoryOf(pid);
    if (!m) continue;
    const cat = category(comm, m.cmdline);
    if (m.pss > topProcess.pssKiB) Object.assign(topProcess, { pssKiB: m.pss, comm, cmdline: m.cmdline.slice(0, 300) });
    byCat[cat] = (byCat[cat] ?? 0) + m.pss;
    counts[cat] = (counts[cat] ?? 0) + 1;
    pss += m.pss;
    rss += m.rss;
  }
  samples += 1;
  for (const [cat, kib] of Object.entries(byCat)) {
    categoryPeakPssKiB[cat] = Math.max(categoryPeakPssKiB[cat] ?? 0, kib);
    categoryMaxProcesses[cat] = Math.max(categoryMaxProcesses[cat] ?? 0, counts[cat]);
  }
  if (pss > peak.pssKiB) {
    Object.assign(peak, {
      pssKiB: pss,
      processes: tree.length,
      atSec: Number(process.hrtime.bigint() - started) / 1e9,
      byCategoryPssKiB: byCat,
    });
  }
  peak.rssKiB = Math.max(peak.rssKiB, rss);
}

const timer = setInterval(sample, intervalMs);
child.on("exit", (code, signal) => {
  clearInterval(timer);
  const mib = (kib) => Math.round(kib / 1024);
  const round = (o) => Object.fromEntries(Object.entries(o).map(([k, v]) => [k, mib(v)]));
  console.log(
    "MEASURE " +
      JSON.stringify({
        label: opts.label ?? command.join(" "),
        exit: code ?? signal,
        elapsedSec: Math.round(Number(process.hrtime.bigint() - started) / 1e8) / 10,
        samples,
        peakTreePssMiB: mib(peak.pssKiB),
        peakTreeRssSumMiB: mib(peak.rssKiB),
        peakAtSec: Math.round(peak.atSec),
        processesAtPeak: peak.processes,
        atPeakPssMiB: round(peak.byCategoryPssKiB),
        categoryPeakPssMiB: round(categoryPeakPssKiB),
        categoryMaxProcesses,
        largestProcess: { pssMiB: mib(topProcess.pssKiB), comm: topProcess.comm, cmdline: topProcess.cmdline.trim() },
      }),
  );
  process.exit(code ?? 1);
});
