# Task 4 report: authenticated production oracles and qualification

## Outcome

Task 4 is operationally complete under the practical internal-experiment
standard. No formal Baseline A, Variant B, controlled A/B, Ultra, or Codex
candidate run was launched.

The production runner executes the exact frozen manifest/oracle-lock mapping,
rejects duplicate command/assertion IDs, requires every placeholder to resolve,
records content-addressed command/log/selector evidence, and fails closed on
nonzero or malformed evidence. Default production use binds all six frozen
controls by a recomputed digest. Every controlling file is required to be a
non-symlink regular file outside the candidate. Manifest and oracle-lock
qualification overrides require their exact byte hashes; benchmark-root
override is forbidden.

## Authenticated exact Rust tests

Exact Rust assertions no longer execute `cargo test` through candidate PATH.
For each locked assertion the runner:

1. validates and hashes an absolute, canonical, non-symlink Cargo binary;
2. derives, validates, hashes, records, and forces Cargo's sibling `rustc`;
3. rejects candidate `.cargo/config*`, `harness = false`, custom test targets,
   Cargo runner environment variables, and Rust compiler/wrapper overrides;
4. obtains trusted Cargo metadata for the selected package/test target;
5. builds with trusted Cargo `--no-run --message-format=json` into the trusted
   artifact directory;
6. selects exactly one emitted test executable and binds its path, byte count,
   SHA-256, Cargo artifact-message hash, package manifest hash, and test-source
   hash;
7. invokes that executable directly with libtest JSON flags.

A pass requires exactly one started event, exactly one `ok` terminal event,
and a successful suite summary proving one passed, zero failed, and zero
ignored. Receipts retain hashes of normalized event and suite projections.
Authenticated Cargo metadata/build nonzero outcomes are recorded as failed
commands and zero-match failed assertions, so real behavior-missing historical
bases discriminate without being mislabeled as trusted-runner failures.
Trust-policy violations still abort before execution.

The initial full rerun exposed a real rustup-shim race: authenticated Cargo
spawned PATH `rustc` shims in parallel and stable-channel component updates
failed with rename/conflict errors. Binding the canonical sibling `rustc`
removed that uncontrolled resolver. A retained TW-03 reproduction then emitted
exactly one expected test artifact, and the all-nine rerun completed.

## Real target/base qualification

`qualify-oracles.mjs` clones isolated target and historical-base workspaces for
all nine cases, verifies base identities, applies the trusted oracle overlay to
both, then executes every frozen core command and every applicable production
oracle command. Commit identity never sets acceptance or discrimination.

| Case | Positive | Negative | Classification |
| --- | --- | --- | --- |
| TW-01 | core pass; packet gate ready | core/oracle executed; packet gate ready | subjective semantics deferred, not machine-qualified |
| TW-02 | core pass; packet gate ready | core/oracle executed; packet gate ready | subjective semantics deferred, not machine-qualified |
| TW-03 | accepted | discriminated | machine-qualified |
| TW-04 | accepted | authenticated missing-package negative discriminated | machine-qualified |
| TW-05 | expected stale-contract miss | discriminated | controlled reference is the positive |
| TW-06 | core pass; packet gate ready | core/oracle executed; packet gate ready | subjective semantics deferred, not machine-qualified |
| TW-07 | accepted | discriminated | machine-qualified |
| TW-08 | accepted | discriminated | machine-qualified |
| TW-09 | accepted | discriminated | machine-qualified |

TW-09 deterministically recreates construction pilot commit
`fdf1963c54254f62f03f46dc936d60baf178b0f8` and tree
`82854a472bd6aca1cab70b750fdcae864675ce5c` from the frozen base plus locked
patch `5bb0d435a779710434b04f8225741a533a2ac79335420d451bffc79aa6fd81cb`.
Its permanent trusted probe compiles against and invokes candidate production
behavior. Its historical base is an executed behavior-missing negative.

## TW-05 calibration and offline execution

Historical target `16289f8` remains an expected calibration miss, never a
positive: its real adapter succeeds, eight of ten assertions pass, and exactly
the frozen stale-revision assertions `tw-05.native-step-4` and
`tw-05.native-step-5` fail. A clearly labeled controlled Rust reference
runtime qualifies the required expected-revision rejection behavior. The
historical base is the executed behavior-missing negative.

The offline runner directly executes the locked native build, real
`wasm32-unknown-unknown` build, Rust tests, Node Worker/parity tests, Node
benchmark, and Bash portability audit. It has no npm, pnpm, or yarn executable
dependency. Every workload runs under `/usr/bin/sandbox-exec` with
`(deny network*)`, Cargo offline mode, and denied proxy variables. A direct
socket probe must receive `EPERM`/`EACCES` first. The receipt binds the sandbox
executable, profile, probe, and workload executable hashes.

The output receipt path must be absolute, prospective-realpath-resolved,
disjoint from the candidate, and nonexistent. Symlinked ancestry, symlink or
special leaves, and existing outputs are rejected; missing directories are
created with restricted permissions and the leaf is reserved with exclusive,
no-follow creation before execution.

## Receipt split and retained evidence

Checked qualification artifact `evaluator/qualifications/oracles.json` contains:

- deterministic summary schema `tachiko-oracle-qualification-summary-v3`;
- deterministic payload SHA-256
  `0da2ad24ccd0bfa13ef4b756f72405b9ae5ffd18d9fb4d9f0246e644c59a31d9`;
- run-specific evidence schema `tachiko-oracle-qualification-run-v3`;
- run-receipt SHA-256
  `b1a8290dd11a26defe16a70d90b61fce1f925b095bf14be4149afe5e9049969e`;
- frozen control digest
  `d72f3793603c719a74cc11a6de1dfaeca10452baeb667a59ec37507d3be40631`;
- construction-pilot-only classification, formal-result eligible false, and
  Codex launched false.

The run receipt retains stdout/stderr byte counts and SHA-256 values,
command-template and resolved-command hashes, JSON-pointer actual-value hashes,
portable selected-record sets/hashes, normalized libtest event/suite hashes,
and adapter observation/log hashes. Raw temporary logs may be deleted after
these digests are retained. Run-specific paths, binary hashes, and raw log
hashes remain only in the content-addressed run receipt. The summary normalizes
those fields and excludes libtest timing. Two actual controlled fixture
qualification runs reproduced its payload byte-for-byte.

## Verification

Fresh commands after receipt generation:

```sh
node --test benchmarks/agents-md-effect/tests/operational.test.mjs
node benchmarks/agents-md-effect/scripts/verify-benchmark.mjs
node --check benchmarks/agents-md-effect/scripts/run-oracles.mjs
node --check benchmarks/agents-md-effect/scripts/qualify-oracles.mjs
node --check benchmarks/agents-md-effect/scripts/run-tw05-offline.mjs
git diff --check
```

Results:

- operational suite: 38 passed, 0 failed;
- benchmark verifier: verified all 9 frozen cases;
- changed-script syntax checks: all exit 0;
- diff check: exit 0;
- checked controls versus current bytes: 25 checked, 0 mismatches;
- all-nine target/base qualification: exit 0;
- deterministic fixture regeneration: two real runs, identical payload bytes.

## Honest limitations

- TW-01, TW-02, and TW-06 semantic discrimination is not machine-qualified;
  only their actual deterministic execution/packet gates are qualified here.
  Deterministic blinded packet fixtures qualify subjective review semantics.
- Direct Cargo/libtest execution is a strong practical internal control, not
  an adversarial sandbox proof against malicious compiler input or build
  scripts running as the experiment user.
- The content-addressed full run receipt is intentionally run-specific; only
  the normalized summary is claimed deterministic.
- Candidate implementations whose internal names differ from the frozen
  adapter contract require one content-addressed trusted adapter while the same
  attempt is paused; this does not authorize resampling.
- Provider-internal immutable deployment identity, additional reviewer panels,
  and further independent neutrality audits remain outside local control.
- Standalone qualification receipts remain construction-only. The run
  controller binds them into an authorized attempt without resampling.

No genuine Task 4 implementation blocker remains.
