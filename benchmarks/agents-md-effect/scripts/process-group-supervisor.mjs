import {spawnSync} from "node:child_process";
import {createHash, createHmac, randomBytes, randomUUID} from "node:crypto";
import {chmod, lstat, mkdtemp, readFile, realpath, rm, writeFile} from "node:fs/promises";
import {tmpdir} from "node:os";
import {dirname, resolve} from "node:path";
import {fileURLToPath} from "node:url";

const DEFAULT_MAX_OUTPUT_BYTES = 128 * 1024 * 1024;
const scriptDir = dirname(fileURLToPath(import.meta.url));
const containmentRunner = resolve(scriptDir, "launchd-contained-runner.mjs");
const coalitionControl = resolve(scriptDir, "process-coalition-control");
const coalitionControlSource = resolve(scriptDir, "process-coalition-control.c");
const coalitionControlLock = resolve(scriptDir, "process-coalition-control-lock.json");
const launchctlExecutable = "/bin/launchctl";
export const PROCESS_GROUP_ESCAPE_DENIAL =
  "(deny syscall-unix (syscall-number SYS_setsid SYS_setpgid))\n" +
  "(deny process-exec (literal \"/bin/launchctl\"))\n" +
  "(deny mach-bootstrap)\n";
export const PROCESS_CONTAINMENT_PROFILE =
  `(version 1)\n(allow default)\n${PROCESS_GROUP_ESCAPE_DENIAL}`;
export const PROCESS_CONTAINMENT_EXECUTABLE = "/usr/bin/sandbox-exec";

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function delay(milliseconds) {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));
}

function monotonicMilliseconds() {
  return Number(process.hrtime.bigint() / 1_000_000n);
}

function canonicalBytes(value) {
  return Buffer.from(`${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function xml(value) {
  return String(value).replaceAll("&", "&amp;").replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;").replaceAll('"', "&quot;").replaceAll("'", "&apos;");
}

function plist({label, runner, specPath, cwd, environment, stdoutPath, stderrPath}) {
  const environmentEntries = Object.entries(environment).sort(([left], [right]) =>
    left.localeCompare(right));
  return Buffer.from(
    `<?xml version="1.0" encoding="UTF-8"?>\n` +
    `<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" ` +
    `"http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n` +
    `<plist version="1.0"><dict>\n` +
    `<key>Label</key><string>${xml(label)}</string>\n` +
    `<key>ProgramArguments</key><array><string>${xml(process.execPath)}</string>` +
    `<string>${xml(runner)}</string><string>${xml(specPath)}</string></array>\n` +
    `<key>WorkingDirectory</key><string>${xml(cwd)}</string>\n` +
    `<key>EnvironmentVariables</key><dict>${environmentEntries.map(([key, value]) =>
      `<key>${xml(key)}</key><string>${xml(value)}</string>`).join("")}</dict>\n` +
    `<key>StandardOutPath</key><string>${xml(stdoutPath)}</string>\n` +
    `<key>StandardErrorPath</key><string>${xml(stderrPath)}</string>\n` +
    `<key>AbandonProcessGroup</key><false/>\n` +
    `<key>RunAtLoad</key><true/>\n` +
    `<key>ProcessType</key><string>Interactive</string>\n` +
    `<key>Umask</key><integer>63</integer>\n` +
    `</dict></plist>\n`,
    "utf8",
  );
}

async function fileIdentity(path) {
  const bytes = await readFile(path);
  return {path, bytes: bytes.length, sha256: sha256(bytes)};
}

async function secureFileIdentity(path) {
  const [metadata, identity] = await Promise.all([lstat(path, {bigint: true}), fileIdentity(path)]);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    throw new Error(`supervisor control is not a regular non-symlink file: ${path}`);
  }
  return {
    ...identity,
    device: metadata.dev.toString(),
    inode: metadata.ino.toString(),
    mode: Number(metadata.mode & 0o777n),
  };
}

async function secureDirectoryIdentity(path) {
  const metadata = await lstat(path, {bigint: true});
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
    throw new Error(`supervisor control root is not a regular directory: ${path}`);
  }
  return {
    path,
    device: metadata.dev.toString(),
    inode: metadata.ino.toString(),
    mode: Number(metadata.mode & 0o777n),
  };
}

function execute(executable, args) {
  return spawnSync(executable, args, {encoding: "utf8", maxBuffer: 8 * 1024 * 1024});
}

function requireCommand(result, label) {
  if (result.error || result.status !== 0) {
    throw new Error(`${label} failed: ${result.error?.message ?? result.stderr ?? result.stdout}`);
  }
  return result;
}

function coalitionInfo(pid) {
  const result = requireCommand(execute(coalitionControl, ["info", String(pid)]),
    "resource coalition identity query");
  const info = JSON.parse(result.stdout);
  if (info.pid !== pid || !/^[1-9][0-9]*$/.test(info.resource_coalition_id ?? "")) {
    throw new Error("resource coalition identity query returned an invalid result");
  }
  return info;
}

function coalitionSnapshot(resourceCoalitionId) {
  const result = requireCommand(execute(coalitionControl, ["members", resourceCoalitionId]),
    "resource coalition member query");
  const snapshot = JSON.parse(result.stdout);
  if (snapshot.resource_coalition_id !== resourceCoalitionId ||
      snapshot.pid_list_complete !== true || !Number.isSafeInteger(snapshot.scan_attempts) ||
      snapshot.scan_attempts <= 0 || snapshot.scan_attempts > 8 ||
      snapshot.stable_complete_scans !== 2 || snapshot.scan_attempts < 2 ||
      !Number.isSafeInteger(snapshot.initial_count_hint) || snapshot.initial_count_hint <= 0 ||
      !Array.isArray(snapshot.scans) || snapshot.scans.length !== snapshot.scan_attempts ||
      snapshot.scans.some((scan) =>
        JSON.stringify(Object.keys(scan ?? {}).sort()) !==
          JSON.stringify(["capacity", "count", "duplicate_pid_entries", "invalid_pid_entries"]) ||
        !Number.isSafeInteger(scan.capacity) || !Number.isSafeInteger(scan.count) ||
        scan.capacity <= 0 || scan.count < 0 || scan.count > scan.capacity ||
        !Number.isSafeInteger(scan.invalid_pid_entries) || scan.invalid_pid_entries < 0 ||
        !Number.isSafeInteger(scan.duplicate_pid_entries) || scan.duplicate_pid_entries < 0) ||
      snapshot.scans.at(-1).count >= snapshot.scans.at(-1).capacity ||
      snapshot.scans.at(-1).duplicate_pid_entries !== 0 ||
      snapshot.scans.at(-2).count >= snapshot.scans.at(-2).capacity ||
      snapshot.scans.at(-2).duplicate_pid_entries !== 0 ||
      snapshot.scans.at(-2).capacity !== snapshot.scans.at(-1).capacity ||
      !Array.isArray(snapshot.pids) || snapshot.pids.some((pid) =>
        !Number.isSafeInteger(pid) || pid <= 0) ||
      new Set(snapshot.pids).size !== snapshot.pids.length ||
      snapshot.pids.some((pid, index) => index > 0 && snapshot.pids[index - 1] >= pid)) {
    throw new Error("resource coalition member query returned an invalid result");
  }
  return snapshot;
}

async function readJsonWhenPresent(path) {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch (error) {
    if (error?.code === "ENOENT" || error instanceof SyntaxError) return null;
    throw error;
  }
}

function protectedControlPolicy(paths, containmentRoot) {
  const quote = (value) => value.replaceAll("\\", "\\\\").replaceAll('"', '\\"');
  const protectedPaths = [
    paths.spec, paths.input, paths.ready, paths.gate, paths.exit, paths.finalize, paths.status,
    paths.stdout, paths.stderr,
    paths.runner_stdout, paths.runner_stderr, paths.plist,
  ];
  return {
    protectedPaths,
    fragment: `${protectedPaths.map((path) =>
      `(deny file-read* (literal "${quote(path)}"))\n` +
      `(deny file-write* (literal "${quote(path)}"))\n`).join("")}` +
      `(deny file-write* (literal "${quote(containmentRoot)}"))\n`,
  };
}

async function readAuthenticatedStatusWhenPresent(path, nonce, authenticationKey) {
  const envelope = await readJsonWhenPresent(path);
  if (envelope === null) return null;
  if (JSON.stringify(Object.keys(envelope).sort()) !==
      JSON.stringify(["authentication", "nonce", "schema", "terminal"]) ||
      envelope.schema !== "tachiko-launchd-contained-status-v1" || envelope.nonce !== nonce ||
      JSON.stringify(Object.keys(envelope.authentication ?? {}).sort()) !==
        JSON.stringify(["algorithm", "hmac_sha256"]) ||
      envelope.authentication.algorithm !== "hmac-sha256" ||
      JSON.stringify(Object.keys(envelope.terminal ?? {}).sort()) !==
        JSON.stringify(["exit_code", "signal", "spawn_error", "stderr", "stdout", "target_pid"]) ||
      (envelope.terminal.target_pid !== null &&
        (!Number.isSafeInteger(envelope.terminal.target_pid) || envelope.terminal.target_pid <= 0)) ||
      (envelope.terminal.exit_code !== null && !Number.isSafeInteger(envelope.terminal.exit_code)) ||
      (envelope.terminal.signal !== null && typeof envelope.terminal.signal !== "string") ||
      (envelope.terminal.spawn_error !== null && typeof envelope.terminal.spawn_error !== "string") ||
      ![envelope.terminal.stdout, envelope.terminal.stderr].every((output) =>
        JSON.stringify(Object.keys(output ?? {}).sort()) === JSON.stringify(["bytes", "sha256"]) &&
        Number.isSafeInteger(output.bytes) && output.bytes >= 0 &&
        /^[0-9a-f]{64}$/.test(output.sha256 ?? ""))) {
    throw new Error("launchd coalition runner status envelope is invalid");
  }
  const expectedHmac = createHmac("sha256", authenticationKey)
    .update(canonicalBytes(envelope.terminal)).digest("hex");
  if (envelope.authentication.hmac_sha256 !== expectedHmac) {
    throw new Error("launchd coalition runner status authentication failed");
  }
  return envelope.terminal;
}

async function readAuthenticatedExitWhenPresent(path, nonce, authenticationKey) {
  const envelope = await readJsonWhenPresent(path);
  if (envelope === null) return null;
  if (JSON.stringify(Object.keys(envelope).sort()) !==
      JSON.stringify(["authentication", "exit", "nonce", "schema"]) ||
      envelope.schema !== "tachiko-launchd-contained-exit-v1" || envelope.nonce !== nonce ||
      JSON.stringify(Object.keys(envelope.authentication ?? {}).sort()) !==
        JSON.stringify(["algorithm", "hmac_sha256"]) ||
      envelope.authentication.algorithm !== "hmac-sha256" ||
      JSON.stringify(Object.keys(envelope.exit ?? {}).sort()) !==
        JSON.stringify(["exit_code", "signal", "spawn_error", "target_pid"]) ||
      (envelope.exit.target_pid !== null &&
        (!Number.isSafeInteger(envelope.exit.target_pid) || envelope.exit.target_pid <= 0)) ||
      (envelope.exit.exit_code !== null && !Number.isSafeInteger(envelope.exit.exit_code)) ||
      (envelope.exit.signal !== null && typeof envelope.exit.signal !== "string") ||
      (envelope.exit.spawn_error !== null && typeof envelope.exit.spawn_error !== "string")) {
    throw new Error("launchd coalition runner exit envelope is invalid");
  }
  const expectedHmac = createHmac("sha256", authenticationKey)
    .update(canonicalBytes(envelope.exit)).digest("hex");
  if (envelope.authentication.hmac_sha256 !== expectedHmac) {
    throw new Error("launchd coalition runner exit authentication failed");
  }
  return envelope.exit;
}

function sendCoalitionSignal(resourceCoalitionId, signal, excludedPids, rounds, membersQuery) {
  const signaled = new Set();
  for (let round = 0; round < rounds; round += 1) {
    const members = membersQuery();
    for (const pid of members) {
      if (excludedPids.has(pid)) continue;
      try {
        process.kill(pid, signal);
        signaled.add(pid);
      } catch (error) {
        if (error?.code !== "ESRCH") throw error;
      }
    }
  }
  return [...signaled].sort((left, right) => left - right);
}

async function waitForCoalitionExtinction(
  resourceCoalitionId, excludedPids, membersQuery, milliseconds = 5000,
) {
  const deadline = monotonicMilliseconds() + milliseconds;
  let members = [];
  do {
    members = membersQuery().filter((pid) => !excludedPids.has(pid));
    if (members.length === 0) return [];
    sendCoalitionSignal(resourceCoalitionId, "SIGKILL", excludedPids, 2, membersQuery);
    await delay(25);
  } while (monotonicMilliseconds() < deadline);
  throw new Error(`resource coalition remained alive after SIGKILL: ${members.join(",")}`);
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
  kernelContainmentProfile,
}) {
  if (!Number.isSafeInteger(timeoutMilliseconds) || timeoutMilliseconds <= 0) {
    throw new Error("timeoutMilliseconds must be a positive safe integer");
  }
  if (!Number.isSafeInteger(terminationGraceMilliseconds) || terminationGraceMilliseconds < 0) {
    throw new Error("terminationGraceMilliseconds must be a non-negative safe integer");
  }
  const effectiveCwd = resolve(cwd ?? environment.PWD ?? process.cwd());
  const requestedContainmentProfile = kernelContainmentProfile ?? PROCESS_CONTAINMENT_PROFILE;
  if (!requestedContainmentProfile.includes(PROCESS_GROUP_ESCAPE_DENIAL.trim())) {
    throw new Error("kernel containment profile does not deny process-group/session and launchd escape");
  }
  const [helperIdentity, helperSourceIdentity, helperLockIdentity, helperLockBytes] =
    await Promise.all([
      fileIdentity(coalitionControl), fileIdentity(coalitionControlSource),
      fileIdentity(coalitionControlLock), readFile(coalitionControlLock),
    ]);
  const helperLock = JSON.parse(helperLockBytes.toString("utf8"));
  if (helperLock.schema !== "tachiko-process-coalition-control-lock-v1" ||
      helperLock.platform !== "darwin-arm64" ||
      helperLock.binary?.bytes !== helperIdentity.bytes ||
      helperLock.binary?.sha256 !== helperIdentity.sha256 ||
      helperLock.source?.bytes !== helperSourceIdentity.bytes ||
      helperLock.source?.sha256 !== helperSourceIdentity.sha256 ||
      helperLock.kernel_api !==
        "proc_pidinfo(PROC_PIDCOALITIONINFO=20)+complete-growing-proc_listallpids") {
    throw new Error("resource coalition control binary differs from its operational lock");
  }
  const containmentRoot = await realpath(
    await mkdtemp(resolve(environment.TMPDIR ?? tmpdir(), "tachiko-coalition-")),
  );
  const nonce = randomUUID();
  const label = `com.tachiko.agents-md.${nonce}`;
  const domain = `gui/${process.getuid()}`;
  const service = `${domain}/${label}`;
  const paths = Object.fromEntries([
    "spec", "input", "ready", "gate", "exit", "finalize", "status", "stdout", "stderr",
    "runner_stdout", "runner_stderr",
  ]
    .map((name) => [name, resolve(containmentRoot, name)]));
  paths.plist = resolve(containmentRoot, `${label}.plist`);
  const controlPolicy = protectedControlPolicy(paths, containmentRoot);
  const containmentProfile = `${requestedContainmentProfile}${controlPolicy.fragment}`;
  const launchedExecutable = PROCESS_CONTAINMENT_EXECUTABLE;
  const launchedArgs = ["-p", containmentProfile, executable, ...args];
  const statusAuthenticationKey = randomBytes(32).toString("hex");
  const startedAt = new Date().toISOString();
  const started = process.hrtime.bigint();
  let timeoutDeadline = null;
  const signalActions = [];
  const containmentActions = [];
  let bootstrapped = false;
  let bootoutReceipt = null;
  let timedOut = false;
  let targetExit = null;
  let status = null;
  let runnerPid = null;
  let resourceCoalitionId = null;
  let initialMembers = [];
  let descendantCleanupRequired = false;
  let terminationDeadline = null;
  let terminationGraceIntervals = 0;
  let plistBytes = null;
  let immutableControlsBeforeLaunch = null;
  const coalitionEnumerationScans = [];
  const queryCoalitionMembers = () => {
    if (resourceCoalitionId === null) throw new Error("resource coalition is not yet registered");
    const snapshot = coalitionSnapshot(resourceCoalitionId);
    coalitionEnumerationScans.push({
      pid_list_complete: snapshot.pid_list_complete,
      scan_attempts: snapshot.scan_attempts,
      stable_complete_scans: snapshot.stable_complete_scans,
      initial_count_hint: snapshot.initial_count_hint,
      scans: snapshot.scans,
      coalition_member_count: snapshot.pids.length,
      coalition_members_sha256: sha256(`${JSON.stringify(snapshot.pids)}\n`),
    });
    return snapshot.pids;
  };
  try {
    await Promise.all([
      writeFile(paths.input, input, {mode: 0o600, flag: "wx"}),
      writeFile(paths.runner_stdout, Buffer.alloc(0), {mode: 0o600, flag: "wx"}),
      writeFile(paths.runner_stderr, Buffer.alloc(0), {mode: 0o600, flag: "wx"}),
    ]);
    const spec = {
      schema: "tachiko-launchd-contained-command-v1",
      nonce,
      executable: launchedExecutable,
      args: launchedArgs,
      cwd: effectiveCwd,
      environment,
      input_path: paths.input,
      ready_path: paths.ready,
      gate_path: paths.gate,
      exit_path: paths.exit,
      finalize_path: paths.finalize,
      status_path: paths.status,
      stdout_path: paths.stdout,
      stderr_path: paths.stderr,
      max_output_bytes: maxOutputBytes,
      status_authentication_key: statusAuthenticationKey,
    };
    await writeFile(paths.spec, canonicalBytes(spec), {mode: 0o600, flag: "wx"});
    plistBytes = plist({
      label, runner: containmentRunner, specPath: paths.spec, cwd: effectiveCwd,
      environment: {
        HOME: environment.HOME ?? dirname(containmentRoot),
        TMPDIR: containmentRoot,
        PATH: "/usr/bin:/bin:/usr/sbin:/sbin",
        LANG: "C.UTF-8",
        LC_ALL: "C.UTF-8",
        TZ: "UTC",
      },
      stdoutPath: paths.runner_stdout, stderrPath: paths.runner_stderr,
    });
    await writeFile(paths.plist, plistBytes, {mode: 0o600, flag: "wx"});
    requireCommand(execute("/usr/bin/plutil", ["-lint", paths.plist]), "launchd coalition plist validation");
    const bootstrap = execute(launchctlExecutable, ["bootstrap", domain, paths.plist]);
    requireCommand(bootstrap, "launchd coalition bootstrap");
    bootstrapped = true;

    const readyDeadline = monotonicMilliseconds() + Math.min(timeoutMilliseconds, 10_000);
    let ready = null;
    while (monotonicMilliseconds() < readyDeadline && ready === null) {
      ready = await readJsonWhenPresent(paths.ready);
      if (ready === null) await delay(10);
    }
    if (ready?.schema !== "tachiko-launchd-contained-ready-v1" || ready.nonce !== nonce ||
        !Number.isSafeInteger(ready.pid) || ready.pid <= 0) {
      const diagnostics = execute(launchctlExecutable, ["print", service]);
      throw new Error(
        `launchd coalition runner did not become ready: ` +
        `${await readFile(paths.runner_stderr, "utf8")} ` +
        `${diagnostics.stderr ?? ""} ${diagnostics.stdout ?? ""}`,
      );
    }
    runnerPid = ready.pid;
    const runnerCoalition = coalitionInfo(runnerPid);
    const controllerCoalition = coalitionInfo(process.pid);
    resourceCoalitionId = runnerCoalition.resource_coalition_id;
    if (resourceCoalitionId === controllerCoalition.resource_coalition_id) {
      throw new Error("launchd did not assign a unique resource coalition");
    }
    initialMembers = queryCoalitionMembers();
    if (!initialMembers.includes(runnerPid)) {
      throw new Error("launchd resource coalition does not contain its gated runner");
    }
    const gateBytes = canonicalBytes({
      schema: "tachiko-launchd-contained-gate-v1", nonce,
      resource_coalition_id: resourceCoalitionId,
    });
    immutableControlsBeforeLaunch = {
      root: await secureDirectoryIdentity(containmentRoot),
      spec: await secureFileIdentity(paths.spec),
      input: await secureFileIdentity(paths.input),
      ready: await secureFileIdentity(paths.ready),
      plist: await secureFileIdentity(paths.plist),
      runner_stdout: await secureFileIdentity(paths.runner_stdout),
      runner_stderr: await secureFileIdentity(paths.runner_stderr),
      gate_planned: {bytes: gateBytes.length, sha256: sha256(gateBytes), mode: 0o600},
    };
    await writeFile(paths.gate, gateBytes, {mode: 0o600, flag: "wx"});
    timeoutDeadline = monotonicMilliseconds() + timeoutMilliseconds;

    while (targetExit === null && monotonicMilliseconds() < timeoutDeadline) {
      targetExit = await readAuthenticatedExitWhenPresent(
        paths.exit, nonce, statusAuthenticationKey,
      );
      if (targetExit === null) await delay(10);
    }
    if (targetExit === null) {
      timedOut = true;
      terminationDeadline = monotonicMilliseconds() + terminationGraceMilliseconds;
      terminationGraceIntervals = 1;
      const pids = sendCoalitionSignal(
        resourceCoalitionId, "SIGTERM", new Set([runnerPid]), 2, queryCoalitionMembers,
      );
      signalActions.push({signal: "SIGTERM", sent_at: new Date().toISOString()});
      containmentActions.push({signal: "SIGTERM", pids, reason: "deadline"});
      while (monotonicMilliseconds() < terminationDeadline) {
        targetExit ??= await readAuthenticatedExitWhenPresent(
          paths.exit, nonce, statusAuthenticationKey,
        );
        sendCoalitionSignal(
          resourceCoalitionId, "SIGTERM", new Set([runnerPid]), 1, queryCoalitionMembers,
        );
        await delay(25);
      }
    } else {
      const residual = queryCoalitionMembers()
        .filter((pid) => pid !== runnerPid && pid !== targetExit.target_pid);
      descendantCleanupRequired = residual.length > 0;
      if (descendantCleanupRequired) {
        terminationDeadline = monotonicMilliseconds() + terminationGraceMilliseconds;
        terminationGraceIntervals = 1;
        const pids = sendCoalitionSignal(
          resourceCoalitionId, "SIGTERM", new Set([runnerPid]), 2, queryCoalitionMembers,
        );
        signalActions.push({signal: "SIGTERM", sent_at: new Date().toISOString()});
        containmentActions.push({signal: "SIGTERM", pids, reason: "surviving_descendant"});
        while (monotonicMilliseconds() < terminationDeadline) {
          sendCoalitionSignal(
            resourceCoalitionId, "SIGTERM", new Set([runnerPid]), 1, queryCoalitionMembers,
          );
          await delay(25);
        }
      }
    }
    const membersBeforeKill = queryCoalitionMembers();
    if (timedOut && targetExit?.target_pid) {
      descendantCleanupRequired ||= membersBeforeKill.some((pid) =>
        pid !== runnerPid && pid !== targetExit.target_pid);
    }
    const survivors = membersBeforeKill
      .filter((pid) => pid !== runnerPid && (timedOut || pid !== targetExit?.target_pid));
    if (survivors.length > 0) {
      const pids = sendCoalitionSignal(
        resourceCoalitionId, "SIGKILL", new Set([runnerPid]), 3, queryCoalitionMembers,
      );
      signalActions.push({signal: "SIGKILL", sent_at: new Date().toISOString()});
      containmentActions.push({signal: "SIGKILL", pids, reason: "grace_expired"});
    }
    if (targetExit === null) {
      const materializationDeadline = monotonicMilliseconds() + 2_000;
      while (targetExit === null && monotonicMilliseconds() < materializationDeadline) {
        targetExit = await readAuthenticatedExitWhenPresent(
          paths.exit, nonce, statusAuthenticationKey,
        );
        if (targetExit === null) await delay(10);
      }
    }
    if (targetExit === null) {
      throw new Error("launchd coalition runner produced no authenticated target exit");
    }
    await waitForCoalitionExtinction(
      resourceCoalitionId, new Set([runnerPid]), queryCoalitionMembers,
    );
    const finalizeBytes = canonicalBytes({
      schema: "tachiko-launchd-contained-finalize-v1",
      nonce,
      resource_coalition_id: resourceCoalitionId,
    });
    await writeFile(paths.finalize, finalizeBytes, {mode: 0o600, flag: "wx"});
    const outputDeadline = monotonicMilliseconds() + 2_000;
    while (status === null && monotonicMilliseconds() < outputDeadline) {
      status = await readAuthenticatedStatusWhenPresent(
        paths.status, nonce, statusAuthenticationKey,
      );
      if (status === null) await delay(10);
    }
    if (status === null) throw new Error("launchd coalition runner produced no authenticated status");
    for (const key of ["target_pid", "exit_code", "signal", "spawn_error"]) {
      if (status[key] !== targetExit[key]) {
        throw new Error("launchd coalition terminal status differs from authenticated target exit");
      }
    }
    const bootout = execute(launchctlExecutable, ["bootout", service]);
    bootoutReceipt = {status: bootout.status, signal: bootout.signal, error: bootout.error?.message ?? null,
      stdout_sha256: sha256(bootout.stdout ?? ""), stderr_sha256: sha256(bootout.stderr ?? "")};
    if (bootout.error || ![0, 3].includes(bootout.status)) {
      throw new Error(`launchd coalition bootout failed: ${bootout.error?.message ?? bootout.stderr}`);
    }
    bootstrapped = false;
    await waitForCoalitionExtinction(
      resourceCoalitionId, new Set(), queryCoalitionMembers,
    );
    const [stdout, stderr, sandboxExecutableIdentity, finalHelperIdentity,
      finalHelperSourceIdentity, finalHelperLockIdentity, runnerIdentity, launchctlIdentity,
      stdoutIdentity, stderrIdentity, exitIdentity, finalizeIdentity, statusIdentity, gateIdentity,
      finalRootIdentity, finalSpecIdentity, finalInputIdentity, finalReadyIdentity,
      finalPlistIdentity, runnerStdoutIdentity, runnerStderrIdentity] =
      await Promise.all([
      readFile(paths.stdout), readFile(paths.stderr), fileIdentity(PROCESS_CONTAINMENT_EXECUTABLE),
      fileIdentity(coalitionControl), fileIdentity(coalitionControlSource),
      fileIdentity(coalitionControlLock),
      fileIdentity(containmentRunner), fileIdentity(launchctlExecutable),
      secureFileIdentity(paths.stdout), secureFileIdentity(paths.stderr),
      secureFileIdentity(paths.exit), secureFileIdentity(paths.finalize),
      secureFileIdentity(paths.status), secureFileIdentity(paths.gate),
      secureDirectoryIdentity(containmentRoot), secureFileIdentity(paths.spec),
      secureFileIdentity(paths.input), secureFileIdentity(paths.ready), secureFileIdentity(paths.plist),
      secureFileIdentity(paths.runner_stdout), secureFileIdentity(paths.runner_stderr),
    ]);
    if (JSON.stringify(finalHelperIdentity) !== JSON.stringify(helperIdentity) ||
        JSON.stringify(finalHelperSourceIdentity) !== JSON.stringify(helperSourceIdentity) ||
        JSON.stringify(finalHelperLockIdentity) !== JSON.stringify(helperLockIdentity)) {
      throw new Error("resource coalition control closure changed during supervised execution");
    }
    if (stdout.length > maxOutputBytes || stderr.length > maxOutputBytes) {
      throw new Error("process output exceeded the capture limit");
    }
    const immutableControlsAfter = {
      root: finalRootIdentity,
      spec: finalSpecIdentity,
      input: finalInputIdentity,
      ready: finalReadyIdentity,
      plist: finalPlistIdentity,
      runner_stdout: runnerStdoutIdentity,
      runner_stderr: runnerStderrIdentity,
    };
    if (JSON.stringify(immutableControlsAfter) !== JSON.stringify({
      root: immutableControlsBeforeLaunch.root,
      spec: immutableControlsBeforeLaunch.spec,
      input: immutableControlsBeforeLaunch.input,
      ready: immutableControlsBeforeLaunch.ready,
      plist: immutableControlsBeforeLaunch.plist,
      runner_stdout: immutableControlsBeforeLaunch.runner_stdout,
      runner_stderr: immutableControlsBeforeLaunch.runner_stderr,
    }) || gateIdentity.bytes !== immutableControlsBeforeLaunch.gate_planned.bytes ||
      gateIdentity.sha256 !== immutableControlsBeforeLaunch.gate_planned.sha256 ||
      gateIdentity.mode !== immutableControlsBeforeLaunch.gate_planned.mode ||
      finalizeIdentity.bytes !== finalizeBytes.length ||
      finalizeIdentity.sha256 !== sha256(finalizeBytes) || finalizeIdentity.mode !== 0o600) {
      throw new Error("launchd supervisor immutable control identity changed during execution");
    }
    if (status.stdout?.bytes !== stdoutIdentity.bytes ||
        status.stdout?.sha256 !== stdoutIdentity.sha256 ||
        status.stderr?.bytes !== stderrIdentity.bytes ||
        status.stderr?.sha256 !== stderrIdentity.sha256) {
      throw new Error("launchd runner output identity does not match authenticated status");
    }
    const sandboxTargetSpawnError = status?.spawn_error === null &&
      status?.exit_code === 71 &&
      /^sandbox-exec: execvp\(\) of '.+' failed: .+$/m.test(stderr.toString("utf8"))
      ? stderr.toString("utf8").trim()
      : null;
    return {
      started_at: startedAt,
      completed_at: new Date().toISOString(),
      duration_seconds: Number(process.hrtime.bigint() - started) / 1_000_000_000,
      exit_code: status?.exit_code ?? null,
      signal: status?.signal ?? (timedOut ? "SIGKILL" : null),
      spawn_error: status?.spawn_error ?? sandboxTargetSpawnError,
      timed_out: timedOut,
      process_group_created: true,
      termination_grace_seconds: terminationGraceMilliseconds / 1000,
      termination_grace_intervals: terminationGraceIntervals,
      termination_deadline_reused_for_cleanup: timedOut,
      termination_signal_sent: signalActions.some((entry) => entry.signal === "SIGTERM"),
      kill_signal_sent: signalActions.some((entry) => entry.signal === "SIGKILL"),
      signal_actions: signalActions,
      descendant_cleanup_required: descendantCleanupRequired,
      process_group_extinct_before_capture: true,
      process_containment: {
        mode: "darwin_launchd_resource_coalition_v1",
        active_for_execution: true,
        kernel_policy_active_for_execution: true,
        setsid_denied: true,
        setpgid_denied: true,
        launchctl_exec_and_mach_bootstrap_denied: true,
        posix_spawn_group_and_session_escape_contained_by_resource_coalition: true,
        nested_sandbox_avoided: true,
        executable: sandboxExecutableIdentity,
        profile: {
          bytes: Buffer.byteLength(containmentProfile), sha256: sha256(containmentProfile),
        },
        requested_profile: {
          bytes: Buffer.byteLength(requestedContainmentProfile),
          sha256: sha256(requestedContainmentProfile),
        },
        control_root: {
          candidate_access_denied: true,
          path_sha256: sha256(containmentRoot),
          protected_basenames: controlPolicy.protectedPaths.map((path) => path.slice(
            path.lastIndexOf("/") + 1,
          )).sort(),
          policy_fragment_sha256: sha256(controlPolicy.fragment),
          immutable_before_launch: immutableControlsBeforeLaunch,
          immutable_after_extinction: immutableControlsAfter,
          gate: gateIdentity,
          exit: exitIdentity,
          finalize: finalizeIdentity,
          status: statusIdentity,
          stdout: stdoutIdentity,
          stderr: stderrIdentity,
          runner_stdout: runnerStdoutIdentity,
          runner_stderr: runnerStderrIdentity,
        },
        status_authentication: {
          algorithm: "hmac-sha256",
          key_commitment_sha256: sha256(statusAuthenticationKey),
          verified: true,
        },
        coalition_control: {
          binary: helperIdentity,
          source: helperSourceIdentity,
          lock: helperLockIdentity,
          build_provenance: helperLock,
          api: helperLock.kernel_api,
          complete_enumeration_required: true,
          enumeration_scans: coalitionEnumerationScans,
        },
        launchd: {
          executable: launchctlIdentity,
          domain,
          label,
          abandon_process_group: false,
          plist_sha256: sha256(plistBytes),
          resource_coalition_id: resourceCoalitionId,
          unique_from_controller: true,
          initial_members: initialMembers,
          final_members: [],
          bootout: bootoutReceipt,
        },
        signal_actions: containmentActions,
      },
      stdout,
      stderr,
    };
  } finally {
    if (bootstrapped) execute(launchctlExecutable, ["bootout", service]);
    if (resourceCoalitionId !== null) {
      try {
        await waitForCoalitionExtinction(
          resourceCoalitionId, new Set(), queryCoalitionMembers, 1000,
        );
      } catch { /* primary error wins */ }
    }
    await chmod(containmentRoot, 0o700).catch(() => {});
    await rm(containmentRoot, {recursive: true, force: true});
  }
}
