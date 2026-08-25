#!/usr/bin/env node

import {spawn} from "node:child_process";
import {createHash, createHmac} from "node:crypto";
import {readFile, writeFile} from "node:fs/promises";

function delay(milliseconds) {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));
}

function canonicalBytes(value) {
  return Buffer.from(`${JSON.stringify(value, null, 2)}\n`, "utf8");
}

if (process.argv.length !== 3) throw new Error("usage: launchd-contained-runner.mjs SPEC");
const spec = JSON.parse(await readFile(process.argv[2], "utf8"));
const required = [
  "schema", "nonce", "executable", "args", "cwd", "environment", "input_path",
  "ready_path", "gate_path", "exit_path", "finalize_path", "status_path",
  "stdout_path", "stderr_path",
  "max_output_bytes", "status_authentication_key",
];
if (spec.schema !== "tachiko-launchd-contained-command-v1" ||
    JSON.stringify(Object.keys(spec).sort()) !== JSON.stringify(required.sort()) ||
    !Array.isArray(spec.args) || spec.args.some((argument) => typeof argument !== "string") ||
    !Number.isSafeInteger(spec.max_output_bytes) || spec.max_output_bytes <= 0 ||
    !/^[0-9a-f]{64}$/.test(spec.status_authentication_key ?? "")) {
  throw new Error("invalid launchd contained command specification");
}
await writeFile(spec.ready_path, canonicalBytes({
  schema: "tachiko-launchd-contained-ready-v1",
  nonce: spec.nonce,
  pid: process.pid,
}), {mode: 0o600, flag: "wx"});
let gate;
for (;;) {
  try {
    gate = JSON.parse(await readFile(spec.gate_path, "utf8"));
    break;
  } catch (error) {
    if (error?.code !== "ENOENT" && !(error instanceof SyntaxError)) throw error;
  }
  await delay(10);
}
if (gate?.schema !== "tachiko-launchd-contained-gate-v1" || gate.nonce !== spec.nonce ||
    gate.resource_coalition_id === undefined) {
  throw new Error("invalid launchd containment gate");
}
const input = await readFile(spec.input_path);
const output = {stdout: [], stderr: [], stdoutBytes: 0, stderrBytes: 0};
const capture = (name, chunk) => {
  const bytesKey = `${name}Bytes`;
  output[bytesKey] += chunk.length;
  if (output[bytesKey] <= spec.max_output_bytes) output[name].push(chunk);
};
const writeTerminal = async (value) => {
    const stdout = Buffer.concat(output.stdout);
    const stderr = Buffer.concat(output.stderr);
    const outputExceeded = output.stdoutBytes > spec.max_output_bytes ||
      output.stderrBytes > spec.max_output_bytes;
    const terminal = {
      ...value,
      spawn_error: outputExceeded
        ? `EOUTPUTLIMIT: process output exceeded ${spec.max_output_bytes} bytes`
        : value.spawn_error,
      stdout: {bytes: stdout.length, sha256: createHash("sha256").update(stdout).digest("hex")},
      stderr: {bytes: stderr.length, sha256: createHash("sha256").update(stderr).digest("hex")},
    };
    await Promise.all([
      writeFile(spec.stdout_path, stdout, {mode: 0o600, flag: "wx"}),
      writeFile(spec.stderr_path, stderr, {mode: 0o600, flag: "wx"}),
    ]);
    const terminalBytes = canonicalBytes(terminal);
    await writeFile(spec.status_path, canonicalBytes({
      schema: "tachiko-launchd-contained-status-v1",
      nonce: spec.nonce,
      terminal,
      authentication: {
        algorithm: "hmac-sha256",
        hmac_sha256: createHmac("sha256", spec.status_authentication_key)
          .update(terminalBytes).digest("hex"),
      },
    }), {mode: 0o600, flag: "wx"});
};
const child = spawn(spec.executable, spec.args, {
  cwd: spec.cwd,
  env: spec.environment,
  detached: false,
  stdio: ["pipe", "pipe", "pipe"],
});
child.stdout.on("data", (chunk) => capture("stdout", chunk));
child.stderr.on("data", (chunk) => capture("stderr", chunk));
const outputClosed = Promise.all([child.stdout, child.stderr].map((stream) =>
  new Promise((resolveClose) => stream.once("close", resolveClose))));
let settleExit;
let exitSettled = false;
const exitPromise = new Promise((resolveExit) => { settleExit = resolveExit; });
const recordExit = (value) => {
  if (exitSettled) return;
  exitSettled = true;
  settleExit(value);
};
child.once("error", (error) => recordExit({
  target_pid: child.pid ?? null,
  exit_code: null,
  signal: null,
  spawn_error: `${error.code ?? "ESPAWN"}: ${error.message}`,
}));
child.once("exit", (code, signal) => recordExit({
  target_pid: child.pid ?? null,
  exit_code: code,
  signal,
  spawn_error: null,
}));
child.stdin.on("error", () => {
  if (Number.isSafeInteger(child.pid)) child.kill("SIGTERM");
});
child.stdin.end(input);
const targetExit = await exitPromise;
const exitBytes = canonicalBytes(targetExit);
await writeFile(spec.exit_path, canonicalBytes({
  schema: "tachiko-launchd-contained-exit-v1",
  nonce: spec.nonce,
  exit: targetExit,
  authentication: {
    algorithm: "hmac-sha256",
    hmac_sha256: createHmac("sha256", spec.status_authentication_key)
      .update(exitBytes).digest("hex"),
  },
}), {mode: 0o600, flag: "wx"});
let finalize;
for (;;) {
  try {
    finalize = JSON.parse(await readFile(spec.finalize_path, "utf8"));
    break;
  } catch (error) {
    if (error?.code !== "ENOENT" && !(error instanceof SyntaxError)) throw error;
  }
  await delay(10);
}
if (finalize?.schema !== "tachiko-launchd-contained-finalize-v1" ||
    finalize.nonce !== spec.nonce ||
    finalize.resource_coalition_id !== gate.resource_coalition_id) {
  throw new Error("invalid launchd containment finalization gate");
}
await outputClosed;
await writeTerminal(targetExit);
// Keep the launchd service leader alive after the target exits so the controller can
// query and extinguish the complete resource coalition before unloading the job.
setInterval(() => {}, 60_000);
