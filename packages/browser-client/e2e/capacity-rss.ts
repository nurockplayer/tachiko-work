// Acceptance sensor only. No browser/product dependency; adapters are injected.
export type RssProcess = { pid: number; type: string };
export type RssResolution = RssProcess & (
  | { state: "measured"; rssBytes: number }
  | { state: "exited"; exitProof: { method: "kill-0"; errno: "ESRCH"; at: number } }
  | { state: "unverified"; reason: string; liveness: "live" | "unknown" }
);
export type RssSample = {
  at: number; completedAt: number;
  enumeration: { status: "complete"; processes: RssProcess[] } | { status: "failed"; reason: string };
  processes: RssResolution[]; complete: boolean; rss: number | null;
};
export type RssAdapters = {
  listProcesses(): Promise<RssProcess[]>;
  readStatus(pid: number): Promise<string>;
  probePid(pid: number): void; // Actual host adapter invokes process.kill(pid, 0).
  now(): number;
};
export class RssMeasurementIncomplete extends Error {
  constructor() { super("BLOCKED_MEASUREMENT: incomplete process RSS; performance UNVERIFIED"); }
}
function parseRss(status: string): number {
  const lines = status.split("\n").filter(line => line.startsWith("VmRSS:"));
  const match = lines.length === 1 ? /^VmRSS:\s+(\d+) kB$/.exec(lines[0]!) : null;
  const bytes = match ? Number(match[1]) * 1024 : NaN;
  if (!Number.isSafeInteger(bytes) || bytes <= 0) throw new Error("rss_parse_failed");
  return bytes;
}
export async function collectRssSample(adapter: RssAdapters): Promise<RssSample> {
  const at = adapter.now();
  let enumeration: RssSample["enumeration"];
  try {
    const processes = await adapter.listProcesses();
    if (!processes.length || processes.some(p => !Number.isSafeInteger(p.pid) || p.pid <= 0 || typeof p.type !== "string" || !p.type) || new Set(processes.map(p => p.pid)).size !== processes.length) {
      throw new Error("invalid_process_inventory");
    }
    enumeration = { status: "complete", processes };
  } catch {
    return { at, completedAt: adapter.now(), enumeration: { status: "failed", reason: "process_enumeration_failed" }, processes: [], complete: false, rss: null };
  }
  const processes: RssResolution[] = await Promise.all(enumeration.processes.map(async p => {
    let reason = "status_read_failed";
    try {
      const status = await adapter.readStatus(p.pid);
      reason = "rss_parse_failed";
      return { ...p, state: "measured" as const, rssBytes: parseRss(status) };
    } catch {
      // ENOENT/read/parse failure alone does not establish exit. Only an actual
      // kill(pid,0) ESRCH at this check permits an explicit exited resolution.
      try {
        adapter.probePid(p.pid);
        return { ...p, state: "unverified" as const, reason, liveness: "live" as const };
      } catch (error) {
        if ((error as { code?: unknown }).code === "ESRCH") {
          return { ...p, state: "exited" as const, exitProof: { method: "kill-0" as const, errno: "ESRCH" as const, at: adapter.now() } };
        }
        return { ...p, state: "unverified" as const, reason, liveness: "unknown" as const };
      }
    }
  }));
  const measured = processes.filter(p => p.state === "measured");
  const total = measured.reduce((sum, p) => sum + p.rssBytes, 0);
  const complete = processes.every(p => p.state !== "unverified") && measured.length > 0 && Number.isSafeInteger(total) && total > 0;
  return { at, completedAt: adapter.now(), enumeration, processes, complete, rss: complete ? total : null };
}
export function requireCompleteRss(sample: RssSample): void {
  if (!sample.complete || sample.rss === null) throw new RssMeasurementIncomplete();
}
