import {createHash} from "node:crypto";
import {spawnSync} from "node:child_process";
import {lstat, mkdir, readFile, readdir, realpath} from "node:fs/promises";
import {isAbsolute, resolve} from "node:path";

export const PROVIDER_AUTH_SCHEMA = "tachiko-provider-auth-qualification-v1";
export const CONTROL_PROMPT = "Return exactly CONTROL_OK. Do not use tools.";

function fail(message) {
  throw new Error(message);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

export function keyringAccountForCodexHome(codexHome) {
  if (typeof codexHome !== "string" || !isAbsolute(codexHome)) {
    fail("CODEX_HOME must be an absolute path before deriving its keyring account");
  }
  return `cli|${sha256(Buffer.from(codexHome, "utf8")).slice(0, 16)}`;
}

export function validateProviderAuthQualification(receipt, expected) {
  if (!receipt || typeof receipt !== "object" || Array.isArray(receipt)) {
    fail("provider auth qualification must be a JSON object");
  }
  if (receipt.schema !== PROVIDER_AUTH_SCHEMA ||
      receipt.classification !== "construction_pilot_only" ||
      receipt.formal_result_eligible !== false) {
    fail("provider auth qualification schema or classification is invalid");
  }
  if (!expected || typeof expected !== "object" || !isAbsolute(expected.runRoot)) {
    fail("expected provider auth run root must be absolute");
  }
  if (receipt.run_root !== expected.runRoot) {
    fail("provider auth qualification run root mismatch");
  }
  const expectedCodexHome = resolve(expected.runRoot, "codex-home");
  if (receipt.codex_home !== expectedCodexHome) {
    fail("provider auth qualification CODEX_HOME mismatch");
  }
  if (receipt.keyring_account !== keyringAccountForCodexHome(expectedCodexHome)) {
    fail("provider auth qualification keyring account mismatch");
  }
  if (!/^[0-9a-f]{64}$/.test(receipt.keychain_path_sha256 ?? "")) {
    fail("provider auth qualification keychain path SHA-256 is invalid");
  }
  const metadata = receipt.keychain_metadata;
  if (!metadata || typeof metadata !== "object" || Array.isArray(metadata) ||
      metadata.relative_path !== "Library/Preferences/com.apple.security.plist" ||
      !Number.isSafeInteger(metadata.bytes) || metadata.bytes < 1 ||
      !/^[0-9a-f]{64}$/.test(metadata.sha256 ?? "") ||
      !Number.isSafeInteger(metadata.mode) || metadata.mode < 0 || metadata.mode > 0o7777) {
    fail("provider auth qualification Keychain metadata identity is invalid");
  }
  if (receipt.codex_binary_sha256 !== expected.codexBinarySha256) {
    fail("provider auth qualification Codex binary mismatch");
  }
  if (!/^[0-9a-f]{64}$/.test(receipt.sandbox_executable_sha256 ?? "") ||
      !/^[0-9a-f]{64}$/.test(receipt.candidate_access_profile_sha256 ?? "")) {
    fail("provider auth qualification candidate sandbox binding is invalid");
  }
  if (receipt.model_id !== expected.modelId ||
      receipt.reasoning_effort !== expected.reasoningEffort) {
    fail("provider auth qualification controlled model or reasoning effort mismatch");
  }
  if (receipt.benchmark_task_supplied !== false) {
    fail("provider auth qualification must not supply a benchmark task");
  }
  if (receipt.prompt !== CONTROL_PROMPT || receipt.final_message !== "CONTROL_OK") {
    fail("provider auth qualification must return exactly CONTROL_OK for the neutral prompt");
  }
  if (receipt.tool_calls !== 0) fail("provider auth qualification must use zero tool calls");
  if (receipt.auth_json_present !== false) {
    fail("provider auth qualification must not create auth.json");
  }
  if (receipt.workspace_entry_count !== 0) {
    fail("provider auth qualification workspace must remain empty");
  }
  if (receipt.run_root_removed !== true) {
    fail("provider auth qualification run root must be removed before formal preparation");
  }
  return receipt;
}

export function parseControlJsonl(text) {
  if (typeof text !== "string") fail("qualification JSONL must be text");
  let finalMessage = "";
  let eventCount = 0;
  const toolItems = new Set();
  for (const [index, line] of text.split(/\r?\n/).entries()) {
    if (!line.trim()) continue;
    let event;
    try {
      event = JSON.parse(line);
    } catch {
      fail(`invalid JSONL event at line ${index + 1}`);
    }
    eventCount += 1;
    const item = event?.item;
    if (event?.type === "item.completed" && item?.type === "agent_message" &&
        typeof item.text === "string") {
      finalMessage = item.text;
    }
    if ((event?.type === "item.started" || event?.type === "item.completed") && item &&
        !["agent_message", "reasoning", "error"].includes(item.type)) {
      toolItems.add(typeof item.id === "string" ? item.id : `${index}:${item.type}`);
    }
  }
  return {final_message: finalMessage, tool_calls: toolItems.size, event_count: eventCount};
}

export function interpretCodexLoginStatus(result) {
  if (!result || typeof result !== "object" || result.error) {
    fail(`Codex keyring login status failed: ${result?.error?.message ?? "spawn failure"}`);
  }
  const stdout = typeof result.stdout === "string" ? result.stdout.trim() : "";
  const stderr = typeof result.stderr === "string" ? result.stderr.trim() : "";
  if (result.status === 0 && stdout === "" && stderr === "Logged in using ChatGPT") {
    return stderr;
  }
  if (result.status !== 0 && stdout === "" && stderr === "Not logged in") {
    return "Not logged in";
  }
  fail(`Codex keyring login status failed: ${stderr || stdout || `exit ${result.status}`}`);
}

async function requireRegularFile(path, label) {
  const canonical = await realpath(path);
  const info = await lstat(canonical);
  if (!info.isFile() || info.isSymbolicLink()) fail(`${label} must be a regular file`);
  return {canonical, info};
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

async function requireEntries(path, expected, label) {
  const entries = (await readdir(path)).sort();
  if (JSON.stringify(entries) !== JSON.stringify([...expected].sort())) {
    fail(`${label} contains unexpected entries: ${entries.join(", ") || "<empty>"}`);
  }
}

export async function prepareFreshHomeForKeyring({
  home,
  keychainPath,
  securityExecutable = "/usr/bin/security",
}) {
  if (!isAbsolute(home) || !isAbsolute(keychainPath) || !isAbsolute(securityExecutable)) {
    fail("keyring bootstrap paths must be absolute");
  }
  const homePath = await realpath(home);
  const homeInfo = await lstat(homePath);
  if (!homeInfo.isDirectory() || homeInfo.isSymbolicLink()) {
    fail("fresh HOME must be a real directory");
  }
  if ((await readdir(homePath)).length !== 0) fail("fresh HOME must be empty");
  const {canonical: keychain} = await requireRegularFile(keychainPath, "operator keychain");
  const {canonical: security} = await requireRegularFile(securityExecutable, "security executable");
  const preferences = resolve(homePath, "Library", "Preferences");
  await mkdir(preferences, {recursive: true, mode: 0o700});
  const securityEnvironment = {
    HOME: homePath,
    CFFIXED_USER_HOME: homePath,
    PATH: "/usr/bin:/bin:/usr/sbin:/sbin",
    ...(process.env.USER ? {USER: process.env.USER} : {}),
    ...(process.env.LOGNAME ? {LOGNAME: process.env.LOGNAME} : {}),
  };
  for (const subcommand of ["list-keychains", "default-keychain"]) {
    const result = spawnSync(security, [subcommand, "-d", "user", "-s", keychain], {
      encoding: "utf8",
      env: securityEnvironment,
      maxBuffer: 1024 * 1024,
    });
    if (result.status !== 0 || result.error) {
      fail(`failed to configure fresh HOME ${subcommand}: ${result.error?.message ?? result.stderr.trim()}`);
    }
  }
  const metadataPath = resolve(preferences, "com.apple.security.plist");
  const {info: metadataInfo} = await requireRegularFile(metadataPath, "keychain metadata");
  await requireEntries(homePath, ["Library"], "fresh HOME");
  await requireEntries(resolve(homePath, "Library"), ["Preferences"], "fresh HOME Library");
  await requireEntries(preferences, ["com.apple.security.plist"], "fresh HOME Preferences");
  const metadataBytes = await readFile(metadataPath);
  return {
    mode: "macos_user_keychain",
    keychain_path_sha256: sha256(Buffer.from(keychain, "utf8")),
    metadata: {
      relative_path: "Library/Preferences/com.apple.security.plist",
      bytes: metadataBytes.length,
      sha256: sha256(metadataBytes),
      mode: metadataInfo.mode & 0o7777,
    },
  };
}

export async function verifyChatGptKeyringStatus({
  codexExecutable,
  codexHome,
  environment,
}) {
  if (!environment || environment.CODEX_HOME !== codexHome || !isAbsolute(codexHome)) {
    fail("keyring status environment must bind the exact absolute CODEX_HOME");
  }
  const authJson = resolve(codexHome, "auth.json");
  if (await pathExists(authJson)) fail("CODEX_HOME auth.json is forbidden before keyring status");
  const {canonical: codex} = await requireRegularFile(codexExecutable, "Codex executable");
  const result = spawnSync(
    codex,
    ["-c", 'cli_auth_credentials_store="keyring"', "login", "status"],
    {encoding: "utf8", env: environment, maxBuffer: 1024 * 1024},
  );
  let status;
  try {
    status = interpretCodexLoginStatus(result);
  } catch (error) {
    fail(`ChatGPT keyring authentication is unavailable: ${error.message}`);
  }
  if (status !== "Logged in using ChatGPT") {
    fail(`ChatGPT keyring authentication is unavailable: ${status}`);
  }
  if (await pathExists(authJson)) fail("CODEX_HOME auth.json is forbidden after keyring status");
  return {
    mode: "os_keyring",
    method: "chatgpt",
    keyring_account: keyringAccountForCodexHome(codexHome),
    auth_json_present: false,
    status_output_sha256: sha256(Buffer.from(result.stderr, "utf8")),
  };
}
