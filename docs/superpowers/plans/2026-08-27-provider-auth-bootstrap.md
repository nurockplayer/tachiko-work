# Provider Authentication Bootstrap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Qualify and bind OS-keyring-backed ChatGPT authentication for each fresh benchmark `HOME`/`CODEX_HOME` without exposing credential bytes or consuming another formal benchmark slot.

**Architecture:** A construction-only qualifier prepares a prospective opaque run path, bootstraps the fresh HOME's credential-free macOS Keychain metadata, signs the path-scoped fresh `CODEX_HOME` in through the existing ChatGPT operator session, and proves the controlled Codex invocation with a neutral `CONTROL_OK` prompt. It removes the construction filesystem while retaining only the OS-keyring record and a non-secret receipt. Formal controller authorization binds that receipt; the controller recreates the same fresh paths, verifies keyring-only ChatGPT status after neutral preflight, rejects any `auth.json`, and launches the unchanged benchmark task only after the auth gate passes.

**Tech Stack:** Node.js 24 built-ins and `node:test`, macOS `/usr/bin/security`, frozen Codex CLI 0.149.0.

**Spec:** `benchmarks/agents-md-effect/PROCEDURES.md`, `benchmarks/agents-md-effect/PROTOCOL.md`, and the provider-auth bootstrap requirements supplied for the invalid first Baseline A wave.

## Global Constraints

- Keep `gpt-5.6-sol`, reasoning effort `high`, all nine cases/tasks/historical bases, scoring/oracle semantics, both `AGENTS.md` variants, and every formal no-resampling rule unchanged.
- Do not execute Baseline A or Variant B; auth qualification always uses the literal neutral prompt `Return exactly CONTROL_OK. Do not use tools.`
- Preserve the failed wave and occupied TW-01 slot as permanently invalid/unsealed.
- Never place credential bytes in a task, workspace, environment variable, receipt, repository file, fresh HOME/CODEX_HOME file, or agent-readable filesystem.
- Require OS keyring storage explicitly and fail closed on missing ChatGPT status, receipt drift, path mismatch, auth-file creation, or a non-neutral qualification result.

---

### Task 1: Auth contract regression tests

**Files:**
- Create: `benchmarks/agents-md-effect/tests/provider-auth.test.mjs`
- Modify: `benchmarks/agents-md-effect/tests/operational.test.mjs`

**Interfaces:**
- Consumes: the frozen environment lock, prospective run-root path, auth qualification receipt, and formal authorization commitments.
- Produces: tests that catch removal of the keyring selector, acceptance of an auth file, receipt/path drift, and a missing formal auth qualification commitment.

- [x] **Step 1: Write failing keyring argument and receipt-validation tests**

```js
assert.ok(frozenFormalAgentArguments(lock).includes('cli_auth_credentials_store="keyring"'));
assert.throws(() => validateProviderAuthQualification(receipt, {
  runRoot: differentRunRoot,
}), /run root/i);
```

- [x] **Step 2: Write a failing preflight test for credential-bearing CODEX_HOME**

```js
await writeFile(resolve(fixture.codexHome, "auth.json"), "credential material\n");
assert.notEqual(runAuthPreflight(fixture).status, 0);
```

- [x] **Step 3: Run the auth-focused tests and confirm failures name the missing auth contract**

Run: `node --test benchmarks/agents-md-effect/tests/provider-auth.test.mjs`

Expected: FAIL because the qualifier/receipt helpers and keyring-only frozen argument do not exist.

### Task 2: Construction-only keyring qualification

**Files:**
- Create: `benchmarks/agents-md-effect/scripts/provider-auth.mjs`
- Create: `benchmarks/agents-md-effect/scripts/qualify-provider-auth.mjs`
- Modify: `benchmarks/agents-md-effect/scripts/run-controller.mjs`

**Interfaces:**
- Produces: `keyringAccountForCodexHome(path)`, credential-free fresh-HOME Keychain setup, exact ChatGPT login-status verification, qualification receipt validation, and a construction-only CLI.
- Receipt schema: `tachiko-provider-auth-qualification-v1`; binds controlled Codex/model/effort, prospective run root, derived keyring account, default-keychain path hash, neutral prompt/final response, zero tool calls, no `auth.json`, unchanged workspace, and successful cleanup.

- [x] **Step 1: Implement the smallest path-scoped keyring helpers**

```js
export function keyringAccountForCodexHome(codexHome) {
  return `cli|${sha256(Buffer.from(codexHome, "utf8")).slice(0, 16)}`;
}
```

- [x] **Step 2: Implement the construction-only qualifier**

The qualifier must create only its prospective opaque root, invoke ChatGPT browser login with `cli_auth_credentials_store="keyring"`, send only the hard-coded control prompt, parse JSONL for exactly `CONTROL_OK` and zero tool calls, prove `auth.json` absent, emit the non-secret receipt outside the root, and remove the qualification filesystem without deleting the Keychain record.

- [x] **Step 3: Run the auth-focused tests**

Run: `node --test benchmarks/agents-md-effect/tests/provider-auth.test.mjs`

Expected: PASS.

### Task 3: Formal runner and neutral preflight binding

**Files:**
- Modify: `benchmarks/agents-md-effect/scripts/preflight-run.mjs`
- Modify: `benchmarks/agents-md-effect/scripts/run-controller.mjs`
- Modify: `benchmarks/agents-md-effect/tests/operational.test.mjs`

**Interfaces:**
- Formal controller consumes: `--provider-auth-qualification <external receipt>` and the receipt SHA-256 committed by `tachiko-formal-run-authorization-v1`.
- Preflight consumes: optional exact Keychain metadata identity for formal keyring mode; construction-smoke behavior remains unchanged.
- Produces: a `provider_auth_preflight` stage before `agent_launch` proving ChatGPT keyring status and absence of `auth.json`.

- [x] **Step 1: Add the failing formal authorization/preflight cases to the existing operational suite**

- [x] **Step 2: Add `cli_auth_credentials_store="keyring"` to the frozen formal arguments without changing model or effort**

- [x] **Step 3: Require and validate the external qualification before registration, rebuild only credential-free Keychain metadata after HOME cleanup, and verify status after neutral preflight**

- [x] **Step 4: Re-run focused auth tests and the complete operational suite**

Run: `node --test benchmarks/agents-md-effect/tests/provider-auth.test.mjs`

Run: `node --test benchmarks/agents-md-effect/tests/operational.test.mjs`

Expected: PASS with formal fixture authorizations binding the new trusted receipt and construction smoke remaining auth-independent.

### Task 4: Qualification, audit, and checkpoint

**Files:**
- Modify: `benchmarks/agents-md-effect/PROCEDURES.md`
- Modify: `benchmarks/agents-md-effect/READINESS.md`
- Modify: `benchmarks/agents-md-effect/AUDIT.md`

**Interfaces:**
- Produces: accurate instructions/evidence for the credential-free keyring bootstrap and a current Baseline A readiness verdict.

- [x] **Step 1: Run the real construction-only auth qualifier against a new prospective run root**

Expected: controlled `gpt-5.6-sol`/high returns exactly `CONTROL_OK`, tool-call count is zero, no credential file exists, and no benchmark task is supplied.

- [x] **Step 2: Run changed-trusted-byte gates**

Run: `node --check` for changed `.mjs` files.

Run: `node benchmarks/agents-md-effect/scripts/verify-benchmark.mjs`

Run: the existing operational test suite and verifier gates required by their output.

- [x] **Step 3: Update READINESS/AUDIT without rewriting the frozen failed-wave evidence**

- [x] **Step 4: Review the diff against every global constraint and create one checkpoint commit**

Commit message: `fix(benchmark): bootstrap Codex auth from keyring`
