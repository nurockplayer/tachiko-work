#!/usr/bin/env node
import {createHash} from "node:crypto";
import {spawnSync} from "node:child_process";
import {
  lstat,
  mkdir,
  readFile,
  readdir,
  realpath,
  rm,
  writeFile,
} from "node:fs/promises";
import {basename, dirname, isAbsolute, relative, resolve} from "node:path";
import {fileURLToPath, pathToFileURL} from "node:url";
import {
  CONTROL_PROMPT,
  interpretCodexLoginStatus,
  keyringAccountForCodexHome,
  parseControlJsonl,
  prepareFreshHomeForKeyring,
  PROVIDER_AUTH_SCHEMA,
  validateProviderAuthQualification,
} from "./provider-auth.mjs";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(scriptDir, "..");
const RUN_ROOT = /^r-[0-9a-f]{32}$/;
const MAX_OUTPUT_BYTES = 128 * 1024 * 1024;

function fail(message) {
  throw new Error(message);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function canonicalBytes(value) {
  return Buffer.from(`${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function usage() {
  console.error(
    "usage: node qualify-provider-auth.mjs --run-root /abs/r-<32hex> " +
      "--receipt /abs/new-receipt.json --model-catalog-file /abs/models.json " +
      "--keychain /abs/login.keychain-db",
  );
  process.exit(2);
}

function parseArgs(argv) {
  if (argv.length % 2 !== 0) usage();
  const allowed = new Set(["run-root", "receipt", "model-catalog-file", "keychain"]);
  const result = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index];
    const value = argv[index + 1];
    if (!flag?.startsWith("--") || value === undefined) usage();
    const key = flag.slice(2);
    if (!allowed.has(key) || result.has(key)) usage();
    result.set(key, value);
  }
  for (const key of allowed) if (!result.has(key)) usage();
  return result;
}

function isInside(candidate, parent) {
  const suffix = relative(parent, candidate);
  return suffix === "" || (!suffix.startsWith("..") && !isAbsolute(suffix));
}

async function prospective(path, label) {
  if (!isAbsolute(path)) fail(`${label} must be absolute`);
  let current = path;
  while (true) {
    try {
      await lstat(current);
      if (current === path) fail(`${label} must not already exist`);
      const canonicalParent = await realpath(current);
      return resolve(canonicalParent, relative(current, path));
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
      const parent = dirname(current);
      if (parent === current) fail(`${label} has no existing parent`);
      current = parent;
    }
  }
}

async function requireRegular(path, label) {
  if (!isAbsolute(path)) fail(`${label} must be absolute`);
  const canonical = await realpath(path);
  const info = await lstat(canonical);
  if (!info.isFile() || info.isSymbolicLink()) fail(`${label} must be a regular file`);
  return canonical;
}

async function pathExists(path) {
  try {
    await lstat(path);
    return true;
  } catch (error) {
    if (error?.code === "ENOENT") return false;
    throw error;
  }
}

function closedEnvironment({home, codexHome, tmp}) {
  return {
    HOME: home,
    CODEX_HOME: codexHome,
    TMPDIR: tmp,
    PATH: "/usr/bin:/bin:/usr/sbin:/sbin",
    LANG: "C.UTF-8",
    LC_ALL: "C.UTF-8",
    TZ: "UTC",
  };
}

function loginStatus(codex, environment) {
  const result = spawnSync(
    codex,
    ["-c", 'cli_auth_credentials_store="keyring"', "login", "status"],
    {encoding: "utf8", env: environment, maxBuffer: 1024 * 1024},
  );
  return interpretCodexLoginStatus(result);
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const runRoot = await prospective(args.get("run-root"), "qualification run root");
  const receiptPath = await prospective(args.get("receipt"), "qualification receipt");
  if (!RUN_ROOT.test(basename(runRoot))) {
    fail("qualification run root must use r-<32-lowercase-hex>");
  }
  if (isInside(receiptPath, runRoot)) fail("qualification receipt must remain outside the run root");
  const modelCatalogSource = await requireRegular(args.get("model-catalog-file"), "model catalog");
  const keychain = await requireRegular(args.get("keychain"), "operator keychain");
  if (isInside(modelCatalogSource, runRoot) || isInside(keychain, runRoot)) {
    fail("qualification inputs must remain outside the prospective run root");
  }

  const lock = JSON.parse(await readFile(resolve(benchmarkDir, "environment-lock.json"), "utf8"));
  const codex = await requireRegular(lock.controlled_agent.codex_binary_path, "frozen Codex binary");
  const codexBytes = await readFile(codex);
  if (sha256(codexBytes) !== lock.controlled_agent.codex_binary_sha256) {
    fail("frozen Codex binary SHA-256 differs from environment-lock.json");
  }
  const {candidateAccessProfile, frozenFormalAgentArguments, stageFormalModelCatalog} = await import(
    pathToFileURL(resolve(scriptDir, "run-controller.mjs")).href
  );
  const sandboxExecutable = await requireRegular(
    "/usr/bin/sandbox-exec",
    "candidate sandbox executable",
  );

  const workspace = resolve(runRoot, "workspace");
  const home = resolve(runRoot, "home");
  const codexHome = resolve(runRoot, "codex-home");
  const tmp = resolve(runRoot, "tmp");
  const stagedModelCatalogPath = resolve(runRoot, "runtime", "model-catalog.json");
  let receipt;
  await mkdir(runRoot, {mode: 0o700});
  try {
    await Promise.all([
      mkdir(workspace, {mode: 0o700}),
      mkdir(home, {mode: 0o700}),
      mkdir(codexHome, {mode: 0o700}),
      mkdir(tmp, {mode: 0o700}),
    ]);
    const keyring = await prepareFreshHomeForKeyring({home, keychainPath: keychain});
    const stagedCatalog = await stageFormalModelCatalog({
      sourcePath: modelCatalogSource,
      destinationPath: stagedModelCatalogPath,
      catalogLock: lock.controlled_agent.bundled_model_catalog,
      modelId: lock.controlled_agent.model_id,
    });
    const environment = closedEnvironment({home, codexHome, tmp});
    const initialStatus = loginStatus(codex, environment);
    if (initialStatus !== "Not logged in") {
      fail("prospective CODEX_HOME already has a keyring login; choose a new opaque run root");
    }

    const login = spawnSync(
      codex,
      ["-c", 'cli_auth_credentials_store="keyring"', "login"],
      {env: environment, stdio: "inherit"},
    );
    if (login.error || login.status !== 0) {
      fail(`ChatGPT keyring login failed: ${login.error?.message ?? `exit ${login.status}`}`);
    }
    if (loginStatus(codex, environment) !== "Logged in using ChatGPT") {
      fail("fresh CODEX_HOME did not retain ChatGPT authentication in the OS keyring");
    }
    const authJson = resolve(codexHome, "auth.json");
    if (await pathExists(authJson)) {
      fail("keyring qualification created forbidden auth.json");
    }

    const formalArguments = frozenFormalAgentArguments(
      lock,
      workspace,
      stagedModelCatalogPath,
    );
    formalArguments.splice(1, 0, "--skip-git-repo-check");
    const candidateAccess = candidateAccessProfile({
      protectedRoots: [benchmarkDir],
      protectedPaths: [modelCatalogSource],
      restrictedRoots: [runRoot],
      allowedReadRoots: [workspace, home, codexHome, tmp, dirname(stagedModelCatalogPath)],
      allowedWriteRoots: [workspace, home, codexHome, tmp],
    });
    const control = spawnSync(sandboxExecutable, [
      "-p", candidateAccess.profile, codex, ...formalArguments,
    ], {
      cwd: workspace,
      encoding: "utf8",
      env: environment,
      input: `${CONTROL_PROMPT}\n`,
      maxBuffer: MAX_OUTPUT_BYTES,
    });
    if (control.error || control.status !== 0) {
      fail(`neutral provider auth control failed: ${control.error?.message ?? control.stderr.trim()}`);
    }
    const parsed = parseControlJsonl(control.stdout);
    if (parsed.final_message !== "CONTROL_OK" || parsed.tool_calls !== 0) {
      fail("neutral provider auth control did not return exactly CONTROL_OK with zero tool calls");
    }
    if (await pathExists(authJson)) {
      fail("neutral provider auth control created forbidden auth.json");
    }
    const workspaceEntries = await readdir(workspace);
    if (workspaceEntries.length !== 0) fail("neutral provider auth control changed its workspace");
    receipt = {
      schema: PROVIDER_AUTH_SCHEMA,
      classification: "construction_pilot_only",
      formal_result_eligible: false,
      benchmark_task_supplied: false,
      run_root: runRoot,
      codex_home: codexHome,
      keyring_account: keyringAccountForCodexHome(codexHome),
      keychain_path_sha256: keyring.keychain_path_sha256,
      keychain_metadata: keyring.metadata,
      codex_binary_sha256: lock.controlled_agent.codex_binary_sha256,
      sandbox_executable_sha256: sha256(await readFile(sandboxExecutable)),
      candidate_access_profile_sha256: candidateAccess.profile_sha256,
      model_id: lock.controlled_agent.model_id,
      reasoning_effort: lock.controlled_agent.reasoning_effort,
      model_catalog_sha256: stagedCatalog.sha256,
      effective_arguments_sha256: sha256(canonicalBytes(formalArguments)),
      prompt: CONTROL_PROMPT,
      final_message: parsed.final_message,
      tool_calls: parsed.tool_calls,
      event_count: parsed.event_count,
      stdout_sha256: sha256(Buffer.from(control.stdout, "utf8")),
      stderr_sha256: sha256(Buffer.from(control.stderr, "utf8")),
      auth_json_present: false,
      workspace_entry_count: workspaceEntries.length,
      run_root_removed: true,
    };
    validateProviderAuthQualification(receipt, {
      runRoot,
      codexBinarySha256: lock.controlled_agent.codex_binary_sha256,
      modelId: lock.controlled_agent.model_id,
      reasoningEffort: lock.controlled_agent.reasoning_effort,
    });
  } finally {
    await rm(runRoot, {recursive: true, force: false});
  }
  try {
    await lstat(runRoot);
    fail("qualification run root still exists after cleanup");
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }
  await writeFile(receiptPath, canonicalBytes(receipt), {mode: 0o600, flag: "wx"});
  console.log(JSON.stringify({
    classification: receipt.classification,
    final_message: receipt.final_message,
    tool_calls: receipt.tool_calls,
    auth_json_present: receipt.auth_json_present,
    run_root_removed: receipt.run_root_removed,
    receipt: receiptPath,
    receipt_sha256: sha256(canonicalBytes(receipt)),
  }));
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
