import {createHash} from "node:crypto";
import {readFile} from "node:fs/promises";
import {dirname, resolve} from "node:path";
import {fileURLToPath} from "node:url";
import {runProcessGroupOnce} from "./process-group-supervisor.mjs";

export const SANDBOX_EXECUTABLE = "/usr/bin/sandbox-exec";
export const DENY_NETWORK_PROFILE = "(version 1)\n(allow default)\n(deny network*)\n";
const scriptDir = dirname(fileURLToPath(import.meta.url));
const probeScript = resolve(scriptDir, "probe-network-denial.mjs");

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function supervision(execution, timeoutMilliseconds) {
  return {
    started_at: execution.started_at,
    completed_at: execution.completed_at,
    duration_seconds: execution.duration_seconds,
    deadline_seconds: timeoutMilliseconds / 1000,
    exit_code: execution.exit_code,
    signal: execution.signal,
    spawn_error: execution.spawn_error,
    timed_out: execution.timed_out,
    process_group_created: execution.process_group_created,
    termination_grace_seconds: execution.termination_grace_seconds,
    termination_grace_intervals: execution.termination_grace_intervals,
    termination_deadline_reused_for_cleanup: execution.termination_deadline_reused_for_cleanup,
    termination_signal_sent: execution.termination_signal_sent,
    kill_signal_sent: execution.kill_signal_sent,
    signal_actions: execution.signal_actions,
    descendant_cleanup_required: execution.descendant_cleanup_required,
    process_group_extinct_before_capture: execution.process_group_extinct_before_capture,
  };
}

export async function probeNetworkSandbox({
  nodeExecutable = process.execPath,
  environment = process.env,
  terminationGraceMilliseconds = 10_000,
} = {}) {
  const [sandboxBytes, probeBytes] = await Promise.all([
    readFile(SANDBOX_EXECUTABLE),
    readFile(probeScript),
  ]);
  const timeoutMilliseconds = 10_000;
  const execution = await runProcessGroupOnce({
    executable: SANDBOX_EXECUTABLE,
    args: ["-p", DENY_NETWORK_PROFILE, nodeExecutable, probeScript],
    environment,
    timeoutMilliseconds,
    terminationGraceMilliseconds,
  });
  const stdout = execution.stdout.toString("utf8");
  if (
    execution.exit_code !== 0 || execution.spawn_error || execution.timed_out ||
    !execution.process_group_extinct_before_capture ||
    !/^network-denied:(?:EPERM|EACCES)\s*$/.test(stdout)
  ) {
    throw new Error(
      `kernel network sandbox active probe failed: ${execution.stderr.toString("utf8") || stdout}`,
    );
  }
  return {
    mode: "darwin_sandbox_deny_network",
    sandbox_executable: {
      path: SANDBOX_EXECUTABLE,
      bytes: sandboxBytes.length,
      sha256: sha256(sandboxBytes),
    },
    profile: {bytes: Buffer.byteLength(DENY_NETWORK_PROFILE), sha256: sha256(DENY_NETWORK_PROFILE)},
    probe_script: {path: probeScript, bytes: probeBytes.length, sha256: sha256(probeBytes)},
    probe_denied: true,
    probe_stdout_sha256: sha256(execution.stdout),
    process_supervision: supervision(execution, timeoutMilliseconds),
  };
}

export async function runNetworkSandboxed({
  executable,
  args = [],
  cwd,
  environment = process.env,
  input = Buffer.alloc(0),
  timeoutMilliseconds,
  terminationGraceMilliseconds,
  maxOutputBytes,
  profile = DENY_NETWORK_PROFILE,
}) {
  const execution = await runProcessGroupOnce({
    executable: SANDBOX_EXECUTABLE,
    args: ["-p", profile, executable, ...args],
    cwd,
    environment,
    input,
    timeoutMilliseconds,
    terminationGraceMilliseconds,
    maxOutputBytes,
  });
  execution.network_sandbox = {
    mode: "darwin_sandbox_deny_network",
    executable_path: SANDBOX_EXECUTABLE,
    profile_sha256: sha256(profile),
  };
  return execution;
}

export function denyReadProfile(roots, {
  allowReadPaths = [],
  allowReadRoots = [],
  denyWriteRoots = [],
  denyWritePaths = [],
  allowWriteRoots = [],
  allowWritePaths = [],
} = {}) {
  const unique = [...new Set(roots.map((root) => resolve(root)))].sort();
  const escaped = unique.map((root) => root.replaceAll("\\", "\\\\").replaceAll('"', '\\"'));
  const allowed = [...new Set(allowReadPaths.map((path) => resolve(path)))].sort()
    .map((path) => path.replaceAll("\\", "\\\\").replaceAll('"', '\\"'));
  const readable = [...new Set(allowReadRoots.map((path) => resolve(path)))].sort()
    .map((path) => path.replaceAll("\\", "\\\\").replaceAll('"', '\\"'));
  const writeDenied = [...new Set([...roots, ...denyWriteRoots].map((path) => resolve(path)))].sort()
    .map((path) => path.replaceAll("\\", "\\\\").replaceAll('"', '\\"'));
  const writeProtected = [...new Set(denyWritePaths.map((path) => resolve(path)))].sort()
    .map((path) => path.replaceAll("\\", "\\\\").replaceAll('"', '\\"'));
  const writableRoots = [...new Set(allowWriteRoots.map((path) => resolve(path)))].sort()
    .map((path) => path.replaceAll("\\", "\\\\").replaceAll('"', '\\"'));
  const writable = [...new Set(allowWritePaths.map((path) => resolve(path)))].sort()
    .map((path) => path.replaceAll("\\", "\\\\").replaceAll('"', '\\"'));
  return `${DENY_NETWORK_PROFILE}${escaped.map((root) =>
    `(deny file-read* (subpath "${root}"))\n`).join("")}${writeDenied.map((root) =>
    `(deny file-write* (subpath "${root}"))\n`).join("")}${writeProtected.map((path) =>
    `(deny file-write* (literal "${path}"))\n`).join("")}${[...new Set([...escaped, ...writeDenied])].map((root) =>
    `(allow file-read-metadata (subpath "${root}"))\n`).join("")}${readable.map((root) =>
    `(allow file-read* (subpath "${root}"))\n`).join("")}${allowed.map((path) =>
    `(allow file-read* (literal "${path}"))\n`).join("")}${writableRoots.map((path) =>
    `(allow file-write* (subpath "${path}"))\n`).join("")}${writable.map((path) =>
    `(allow file-write* (literal "${path}"))\n`).join("")}`;
}
