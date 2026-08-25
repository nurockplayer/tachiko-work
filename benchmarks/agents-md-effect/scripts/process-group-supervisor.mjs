import {spawn} from "node:child_process";

const DEFAULT_MAX_OUTPUT_BYTES = 128 * 1024 * 1024;

function delay(milliseconds) {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));
}

function monotonicMilliseconds() {
  return Number(process.hrtime.bigint() / 1_000_000n);
}

function processGroupAlive(groupId) {
  try {
    process.kill(-groupId, 0);
    return true;
  } catch (error) {
    if (error?.code === "ESRCH") return false;
    if (error?.code === "EPERM") return true;
    throw error;
  }
}

async function extinguishProcessGroup(
  groupId,
  terminationGraceMilliseconds,
  existingDeadline,
  sendSignal,
) {
  if (!Number.isSafeInteger(groupId) || groupId <= 0) {
    return {cleanup_required: false, deadline_reused: false, grace_interval_started: false};
  }
  const cleanupRequired = processGroupAlive(groupId);
  if (!cleanupRequired) {
    return {
      cleanup_required: false,
      deadline_reused: existingDeadline !== null,
      grace_interval_started: false,
    };
  }
  const deadline = existingDeadline ?? monotonicMilliseconds() + terminationGraceMilliseconds;
  if (existingDeadline === null) {
    sendSignal(groupId, "SIGTERM");
  }
  while (monotonicMilliseconds() < deadline && processGroupAlive(groupId)) {
    await delay(25);
  }
  if (processGroupAlive(groupId)) {
    sendSignal(groupId, "SIGKILL");
  }
  for (let elapsed = 0; elapsed < 5000 && processGroupAlive(groupId); elapsed += 25) {
    await delay(25);
  }
  if (processGroupAlive(groupId)) {
    throw new Error("process group remained alive after SIGKILL");
  }
  return {
    cleanup_required: true,
    deadline_reused: existingDeadline !== null,
    grace_interval_started: existingDeadline === null,
  };
}

export async function runProcessGroupOnce({
  executable,
  args,
  cwd,
  environment,
  input = Buffer.alloc(0),
  timeoutMilliseconds,
  terminationGraceMilliseconds,
  maxOutputBytes = DEFAULT_MAX_OUTPUT_BYTES,
}) {
  if (!Number.isSafeInteger(timeoutMilliseconds) || timeoutMilliseconds <= 0) {
    throw new Error("timeoutMilliseconds must be a positive safe integer");
  }
  if (!Number.isSafeInteger(terminationGraceMilliseconds) || terminationGraceMilliseconds < 0) {
    throw new Error("terminationGraceMilliseconds must be a non-negative safe integer");
  }
  let spawnError = null;
  let timedOut = false;
  let overflow = false;
  const stdoutChunks = [];
  const stderrChunks = [];
  let stdoutBytes = 0;
  let stderrBytes = 0;
  const startedAt = new Date().toISOString();
  const started = process.hrtime.bigint();
  const signalActions = [];
  const sendSignal = (groupId, signal) => {
    try {
      process.kill(-groupId, signal);
      signalActions.push({signal, sent_at: new Date().toISOString()});
      return true;
    } catch (error) {
      if (error?.code === "ESRCH") return false;
      throw error;
    }
  };
  const child = spawn(executable, args, {
    cwd,
    env: environment,
    detached: true,
    stdio: ["pipe", "pipe", "pipe"],
  });
  child.stdout.on("data", (chunk) => {
    stdoutBytes += chunk.length;
    if (stdoutBytes <= maxOutputBytes) stdoutChunks.push(Buffer.from(chunk));
    else overflow = true;
  });
  child.stderr.on("data", (chunk) => {
    stderrBytes += chunk.length;
    if (stderrBytes <= maxOutputBytes) stderrChunks.push(Buffer.from(chunk));
    else overflow = true;
  });
  child.on("error", (error) => { spawnError = error.message; });
  child.stdin.on("error", (error) => { if (!spawnError) spawnError = error.message; });
  child.stdin.end(input);
  const completion = new Promise((resolveCompletion) => {
    child.once("close", (code, signal) => resolveCompletion({code, signal}));
  });
  let killTimer = null;
  let terminationDeadline = null;
  let terminationGraceIntervals = 0;
  const timer = setTimeout(() => {
    timedOut = true;
    if (!Number.isSafeInteger(child.pid) || child.pid <= 0) return;
    terminationDeadline = monotonicMilliseconds() + terminationGraceMilliseconds;
    terminationGraceIntervals = 1;
    sendSignal(child.pid, "SIGTERM");
    killTimer = setTimeout(() => {
      sendSignal(child.pid, "SIGKILL");
    }, terminationGraceMilliseconds);
  }, timeoutMilliseconds);
  const {code, signal} = await completion;
  clearTimeout(timer);
  if (killTimer) clearTimeout(killTimer);
  const processGroupCreated = Number.isSafeInteger(child.pid) && child.pid > 0;
  const cleanup = await extinguishProcessGroup(
    child.pid,
    terminationGraceMilliseconds,
    terminationDeadline,
    sendSignal,
  );
  if (cleanup.grace_interval_started) terminationGraceIntervals += 1;
  if (overflow) throw new Error("process output exceeded the capture limit");
  return {
    started_at: startedAt,
    completed_at: new Date().toISOString(),
    duration_seconds: Number(process.hrtime.bigint() - started) / 1_000_000_000,
    exit_code: code,
    signal,
    spawn_error: spawnError,
    timed_out: timedOut,
    process_group_created: processGroupCreated,
    termination_grace_seconds: terminationGraceMilliseconds / 1000,
    termination_grace_intervals: terminationGraceIntervals,
    termination_deadline_reused_for_cleanup: cleanup.deadline_reused,
    termination_signal_sent: signalActions.some((entry) => entry.signal === "SIGTERM"),
    kill_signal_sent: signalActions.some((entry) => entry.signal === "SIGKILL"),
    signal_actions: signalActions,
    descendant_cleanup_required: cleanup.cleanup_required,
    process_group_extinct_before_capture: true,
    stdout: Buffer.concat(stdoutChunks),
    stderr: Buffer.concat(stderrChunks),
  };
}
