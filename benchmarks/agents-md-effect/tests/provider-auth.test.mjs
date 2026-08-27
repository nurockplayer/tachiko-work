import assert from "node:assert/strict";
import {chmod, mkdir, mkdtemp, readFile, rm, writeFile} from "node:fs/promises";
import {test} from "node:test";
import {dirname, join, resolve} from "node:path";
import {tmpdir} from "node:os";
import {fileURLToPath, pathToFileURL} from "node:url";

const testDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(testDir, "..");
const controllerPath = resolve(benchmarkDir, "scripts/run-controller.mjs");
const providerAuthPath = resolve(benchmarkDir, "scripts/provider-auth.mjs");

test("formal Codex arguments pin OS-keyring authentication without changing model or effort", async () => {
  const {frozenFormalAgentArguments} = await import(pathToFileURL(controllerPath));
  assert.equal(typeof frozenFormalAgentArguments, "function");
  const lock = JSON.parse(await readFile(resolve(benchmarkDir, "environment-lock.json"), "utf8"));
  const arguments_ = frozenFormalAgentArguments(lock);

  assert.ok(arguments_.includes('cli_auth_credentials_store="keyring"'));
  assert.equal(arguments_[arguments_.indexOf("--model") + 1], "gpt-5.6-sol");
  assert.ok(arguments_.includes('model_reasoning_effort="high"'));
});

test("provider auth qualification binds the prospective path and excludes credential files", async () => {
  const {keyringAccountForCodexHome, validateProviderAuthQualification} =
    await import(pathToFileURL(providerAuthPath));
  const runRoot = "/private/tmp/r-0123456789abcdef0123456789abcdef";
  const codexHome = `${runRoot}/codex-home`;
  const expected = {
    runRoot,
    codexBinarySha256: "a".repeat(64),
    modelId: "gpt-5.6-sol",
    reasoningEffort: "high",
  };
  const receipt = {
    schema: "tachiko-provider-auth-qualification-v1",
    classification: "construction_pilot_only",
    formal_result_eligible: false,
    benchmark_task_supplied: false,
    run_root: runRoot,
    codex_home: codexHome,
    keyring_account: "cli|98caf4172b49a794",
    keychain_path_sha256: "b".repeat(64),
    keychain_metadata: {
      relative_path: "Library/Preferences/com.apple.security.plist",
      bytes: 128,
      sha256: "c".repeat(64),
      mode: 0o644,
    },
    codex_binary_sha256: expected.codexBinarySha256,
    sandbox_executable_sha256: "d".repeat(64),
    candidate_access_profile_sha256: "e".repeat(64),
    model_id: expected.modelId,
    reasoning_effort: expected.reasoningEffort,
    prompt: "Return exactly CONTROL_OK. Do not use tools.",
    final_message: "CONTROL_OK",
    tool_calls: 0,
    auth_json_present: false,
    workspace_entry_count: 0,
    run_root_removed: true,
  };

  assert.equal(keyringAccountForCodexHome(codexHome), "cli|98caf4172b49a794");
  assert.doesNotThrow(() => validateProviderAuthQualification(receipt, expected));
  for (const [field, value, pattern] of [
    ["run_root", "/private/tmp/r-fedcba9876543210fedcba9876543210", /run root/i],
    ["auth_json_present", true, /auth\.json/i],
    ["benchmark_task_supplied", true, /benchmark task/i],
    ["formal_result_eligible", true, /classification/i],
    ["final_message", "NOT_OK", /CONTROL_OK/i],
    ["tool_calls", 1, /tool call/i],
    ["run_root_removed", false, /removed/i],
  ]) {
    assert.throws(
      () => validateProviderAuthQualification({...receipt, [field]: value}, expected),
      pattern,
    );
  }
  assert.throws(
    () => validateProviderAuthQualification({
      ...receipt,
      keychain_metadata: {...receipt.keychain_metadata, relative_path: "auth.json"},
    }, expected),
    /Keychain metadata/i,
  );
});

test("fresh HOME keyring setup admits only credential-free Keychain metadata", async () => {
  const {prepareFreshHomeForKeyring} = await import(pathToFileURL(providerAuthPath));
  const root = await mkdtemp(join(tmpdir(), "tachiko-provider-auth-"));
  try {
    const home = resolve(root, "home");
    const keychain = resolve(root, "operator.keychain-db");
    const fakeSecurity = resolve(root, "security");
    await mkdir(home);
    await writeFile(keychain, "non-secret fixture\n");
    await writeFile(fakeSecurity, [
      "#!/bin/sh",
      "set -eu",
      "test \"$2\" = \"-d\"",
      "test \"$3\" = \"user\"",
      "test \"$4\" = \"-s\"",
      "printf '%s\\n' \"$5\" > \"$HOME/Library/Preferences/com.apple.security.plist\"",
      "",
    ].join("\n"));
    await chmod(fakeSecurity, 0o755);

    const result = await prepareFreshHomeForKeyring({
      home,
      keychainPath: keychain,
      securityExecutable: fakeSecurity,
    });
    assert.equal(result.metadata.relative_path, "Library/Preferences/com.apple.security.plist");
    assert.match(result.metadata.sha256, /^[0-9a-f]{64}$/);
    assert.match(result.keychain_path_sha256, /^[0-9a-f]{64}$/);

    const contaminatedHome = resolve(root, "contaminated-home");
    await mkdir(contaminatedHome);
    await writeFile(resolve(contaminatedHome, "auth.json"), "must not survive\n");
    await assert.rejects(
      prepareFreshHomeForKeyring({
        home: contaminatedHome,
        keychainPath: keychain,
        securityExecutable: fakeSecurity,
      }),
      /fresh HOME must be empty/i,
    );
  } finally {
    await rm(root, {recursive: true, force: true});
  }
});

test("neutral qualification JSONL distinguishes the final response from tool activity", async () => {
  const {parseControlJsonl} = await import(pathToFileURL(providerAuthPath));
  const clean = [
    {type: "thread.started", thread_id: "fixture"},
    {type: "turn.started"},
    {type: "item.completed", item: {id: "item_0", type: "agent_message", text: "CONTROL_OK"}},
    {type: "turn.completed", usage: {input_tokens: 1, output_tokens: 1}},
  ].map(JSON.stringify).join("\n");
  assert.deepEqual(parseControlJsonl(`${clean}\n`), {
    final_message: "CONTROL_OK",
    tool_calls: 0,
    event_count: 4,
  });

  const withTool = `${clean}\n${JSON.stringify({
    type: "item.completed",
    item: {id: "item_1", type: "command_execution", command: "true"},
  })}\n`;
  assert.equal(parseControlJsonl(withTool).tool_calls, 1);
  assert.throws(() => parseControlJsonl("not-json\n"), /invalid JSONL/i);
});

test("Codex login status accepts the CLI's unauthenticated nonzero exit contract", async () => {
  const {interpretCodexLoginStatus} = await import(pathToFileURL(providerAuthPath));
  assert.equal(interpretCodexLoginStatus({
    status: 1,
    stdout: "",
    stderr: "Not logged in\n",
  }), "Not logged in");
  assert.equal(interpretCodexLoginStatus({
    status: 0,
    stdout: "",
    stderr: "Logged in using ChatGPT\n",
  }), "Logged in using ChatGPT");
  assert.throws(
    () => interpretCodexLoginStatus({status: 1, stdout: "", stderr: "HTTP 401\n"}),
    /HTTP 401/,
  );
});

test("formal keyring status accepts only ChatGPT auth and never an auth.json side channel", async () => {
  const {verifyChatGptKeyringStatus} = await import(pathToFileURL(providerAuthPath));
  const root = await mkdtemp(join(tmpdir(), "tachiko-provider-status-"));
  try {
    const codexHome = resolve(root, "codex-home");
    const home = resolve(root, "home");
    await Promise.all([mkdir(codexHome), mkdir(home)]);
    const environment = {
      HOME: home,
      CODEX_HOME: codexHome,
      PATH: "/usr/bin:/bin:/usr/sbin:/sbin",
    };
    const fakeCodex = resolve(root, "codex");
    await writeFile(
      fakeCodex,
      "#!/bin/sh\nprintf '%s\\n' 'Logged in using ChatGPT' >&2\n",
      {mode: 0o755},
    );
    const status = await verifyChatGptKeyringStatus({
      codexExecutable: fakeCodex,
      codexHome,
      environment,
    });
    assert.equal(status.method, "chatgpt");
    assert.equal(status.auth_json_present, false);

    await writeFile(fakeCodex, "#!/bin/sh\nprintf '%s\\n' 'Not logged in' >&2\nexit 1\n", {
      mode: 0o755,
    });
    await assert.rejects(
      verifyChatGptKeyringStatus({codexExecutable: fakeCodex, codexHome, environment}),
      /ChatGPT keyring authentication is unavailable/i,
    );

    await writeFile(resolve(codexHome, "auth.json"), "forbidden\n");
    await assert.rejects(
      verifyChatGptKeyringStatus({codexExecutable: fakeCodex, codexHome, environment}),
      /auth\.json is forbidden/i,
    );
  } finally {
    await rm(root, {recursive: true, force: true});
  }
});
