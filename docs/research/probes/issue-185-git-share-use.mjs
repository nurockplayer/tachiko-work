#!/usr/bin/env node
// DISPOSABLE, NON-PRODUCTION research probe for Issue #185.
//
// This executable evidence uses only Node built-ins, ordinary Git, and the
// current Tachiko CLI. It deliberately does not define a package manifest,
// registry protocol, updater, trust model, or stable reusable-asset format.

import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  access,
  cp,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const SCRIPT_DIRECTORY = dirname(fileURLToPath(import.meta.url));
const REPOSITORY_ROOT = resolve(SCRIPT_DIRECTORY, '../../..');
const MOONFALL_SOURCE = join(
  REPOSITORY_ROOT,
  'examples/game-balance/game-balance.ro',
);
const TACHIKO_BINARY = process.env.TACHIKO_BIN
  ? resolve(process.env.TACHIKO_BIN)
  : join(REPOSITORY_ROOT, 'target/debug/tachiko');

const PAYLOAD_PATH = 'asset/game-balance.ro';
const SOURCE_REFERENCE = 'ordinary-git:issue-185-runtime-publisher';
const COMMIT_ENVIRONMENT = Object.freeze({
  GIT_AUTHOR_NAME: 'Tachiko E9 Probe',
  GIT_AUTHOR_EMAIL: 'issue-185@tachiko.invalid',
  GIT_COMMITTER_NAME: 'Tachiko E9 Probe',
  GIT_COMMITTER_EMAIL: 'issue-185@tachiko.invalid',
});

const {
  GIT_DIR: _gitDirectory,
  GIT_WORK_TREE: _gitWorkTree,
  GIT_COMMON_DIR: _gitCommonDirectory,
  GIT_INDEX_FILE: _gitIndexFile,
  GIT_OBJECT_DIRECTORY: _gitObjectDirectory,
  GIT_ALTERNATE_OBJECT_DIRECTORIES: _gitAlternateObjectDirectories,
  ...CLEAN_PROCESS_ENVIRONMENT
} = process.env;

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function run(command, args, options = {}) {
  return execFileSync(command, args, {
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
    ...options,
  });
}

function runBytes(command, args, options = {}) {
  return execFileSync(command, args, {
    maxBuffer: 16 * 1024 * 1024,
    ...options,
  });
}

function git(repository, args, options = {}) {
  return run('git', ['-C', repository, ...args], {
    env: CLEAN_PROCESS_ENVIRONMENT,
    ...options,
  });
}

function gitBytes(repository, args, options = {}) {
  return runBytes('git', ['-C', repository, ...args], {
    env: CLEAN_PROCESS_ENVIRONMENT,
    ...options,
  });
}

function tachiko(args) {
  return run(TACHIKO_BINARY, args);
}

function commit(repository, message, timestamp) {
  git(repository, ['add', '--all']);
  git(repository, ['commit', '--quiet', '-m', message], {
    env: {
      ...CLEAN_PROCESS_ENVIRONMENT,
      ...COMMIT_ENVIRONMENT,
      GIT_AUTHOR_DATE: timestamp,
      GIT_COMMITTER_DATE: timestamp,
    },
  });
  const commitSha = git(repository, ['rev-parse', 'HEAD']).trim();
  assert.match(commitSha, /^[0-9a-f]{40}$/, 'probe requires exact SHA-1 Git identity');
  return commitSha;
}

async function pathExists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

function resolveTuple(tuple) {
  assert.match(tuple.commitSha, /^[0-9a-f]{40}$/);
  assert.match(tuple.payloadDigest, /^[0-9a-f]{64}$/);
  const result = spawnSync(
    'git',
    ['-C', tuple.repository, 'show', `${tuple.commitSha}:${tuple.payloadPath}`],
    { env: CLEAN_PROCESS_ENVIRONMENT, maxBuffer: 16 * 1024 * 1024 },
  );

  if (result.status !== 0) {
    return { available: false, reason: 'immutable_payload_unresolvable' };
  }

  const actualDigest = sha256(result.stdout);
  if (actualDigest !== tuple.payloadDigest) {
    return {
      available: false,
      reason: 'payload_digest_mismatch',
      actualDigest,
    };
  }

  return { available: true, bytes: result.stdout, actualDigest };
}

async function surfaceDiscoveryHint(tuple, destination) {
  const resolution = resolveTuple(tuple);
  if (!resolution.available) return resolution;

  await writeFile(
    destination,
    `EXPERIMENT-ONLY DISCOVERY HINT (NON-AUTHORITATIVE)
source repository reference: ${tuple.sourceReference}
exact Git commit SHA: ${tuple.commitSha}
payload path: ${tuple.payloadPath}
payload SHA-256: ${tuple.payloadDigest}
moving branch: main (discovery only; never a consumable identity)
`,
  );
  return resolution;
}

function validateDeterministically(path, label) {
  const first = tachiko(['validate', path]);
  const second = tachiko(['validate', path]);
  assert.equal(second, first, `${label} validation output must be deterministic`);
  return first;
}

function assertContains(haystack, needle, label) {
  assert.ok(haystack.includes(needle), `${label} must include ${JSON.stringify(needle)}`);
}

async function main() {
  await access(TACHIKO_BINARY);
  const sourceBefore = await readFile(MOONFALL_SOURCE);
  const sourceStatusBefore = git(REPOSITORY_ROOT, [
    'status',
    '--porcelain',
    '--',
    'examples/game-balance/game-balance.ro',
  ]);

  const probeRoot = await mkdtemp(join(tmpdir(), 'tachiko-issue-185-'));
  const keepTemporaryArea = process.env.KEEP_E9_TMP === '1';

  try {
    const publisherWorkspace = join(probeRoot, 'publisher-workspace');
    const publisherRepository = join(probeRoot, 'publisher-repository');
    const consumerWorkspace = join(probeRoot, 'consumer-workspace');
    const inspectionDirectory = join(consumerWorkspace, 'inspection');
    const vendoredDirectory = join(consumerWorkspace, 'project', 'vendor');
    const remixRepository = join(probeRoot, 'remix-repository');

    await mkdir(join(publisherWorkspace, '.tachiko', 'cache'), { recursive: true });
    await mkdir(join(publisherWorkspace, 'private', 'history'), { recursive: true });
    await mkdir(join(publisherWorkspace, 'local'), { recursive: true });
    await writeFile(join(publisherWorkspace, '.env'), 'TOKEN=must-not-be-shared\n');
    await writeFile(
      join(publisherWorkspace, '.tachiko', 'cache', 'session.json'),
      '{"private":"cache"}\n',
    );
    await writeFile(
      join(publisherWorkspace, 'private', 'history', 'notes.md'),
      'private history sentinel\n',
    );
    await writeFile(
      join(publisherWorkspace, 'local', 'absolute-paths.txt'),
      '/Users/example/private/project\n',
    );
    await writeFile(
      join(publisherWorkspace, 'share-preview.txt'),
      [
        'EXPERIMENT-ONLY ALLOWLIST PREVIEW',
        'examples/game-balance/game-balance.ro -> asset/game-balance.ro',
        'generated README.md -> README.md',
        'generated SOURCE-LICENSE.md -> SOURCE-LICENSE.md',
        'LICENSE-APACHE -> LICENSE-APACHE',
        'LICENSE-MIT -> LICENSE-MIT',
        '',
      ].join('\n'),
    );

    // Export only the explicit allowlist. The publisher workspace itself is
    // never copied recursively into the share repository.
    await mkdir(join(publisherRepository, 'asset'), { recursive: true });
    await cp(MOONFALL_SOURCE, join(publisherRepository, PAYLOAD_PATH));
    await cp(join(REPOSITORY_ROOT, 'LICENSE-APACHE'), join(publisherRepository, 'LICENSE-APACHE'));
    await cp(join(REPOSITORY_ROOT, 'LICENSE-MIT'), join(publisherRepository, 'LICENSE-MIT'));
    await writeFile(
      join(publisherRepository, 'README.md'),
      `# Moonfall game-balance example — Issue #185 experiment

This descriptor is experiment-only research evidence. It is not a Tachiko
manifest, package format, compatibility promise, trust signal, or support
contract.

- Artifact class: whole-project/template-style game-balance example.
- Contributes: four schemas (characters, economy, items, weapons), four entities
  (Alric, Iron Sword, Shop, Tempered Blade), three formulas (DPS and economy
  calculations), and typed weapon/item references.
- Payload path: \`${PAYLOAD_PATH}\`.
- Source: Tachiko Work's checked-in Moonfall example; see SOURCE-LICENSE.md.
- License: upstream repository declares Apache-2.0 OR MIT; whether and how that
  declaration applies to a future reusable-asset ecosystem remains unresolved.
- Tachiko assumptions: the current repository CLI can validate, set, diff, and
  three-way merge this \`.ro\` document. No future compatibility is promised.
- Required semantic capabilities: read/validate, explicit set, semantic diff,
  and explicit three-way merge. These are requirements, never capability grants.
- Executable or externally effectful content: none.
- Reusable-asset dependencies: none. In-document references and formulas are
  ordinary Tachiko semantics, not ecosystem dependencies.
- Deterministic conformance evidence: run
  \`TACHIKO_BIN=/path/to/tachiko node docs/research/probes/issue-185-git-share-use.mjs\`.
- Trust boundary: Git hosting or listing does not imply verification, support,
  compatibility certification, signing, trust, approval, or semantic authority.
- Relationships in this experiment: Use/reference is not implemented;
  Copy/Vendor is an independent copy of one immutable payload; Fork/Remix is an
  independent derivative that preserves human-readable ancestry only.
`,
    );
    await writeFile(
      join(publisherRepository, 'SOURCE-LICENSE.md'),
      `# Source and license references

- Source repository: https://github.com/nurockplayer/tachiko-work
- Source path: examples/game-balance/game-balance.ro
- Repository license declaration: Apache-2.0 OR MIT
- Included license texts: LICENSE-APACHE and LICENSE-MIT

This experiment makes those facts inspectable. It does not decide future asset
license applicability, redistribution rights, provenance policy, or support.
`,
    );

    run(
      'git',
      [
        'init',
        '--quiet',
        '--initial-branch=main',
        '--object-format=sha1',
        publisherRepository,
      ],
      { env: CLEAN_PROCESS_ENVIRONMENT },
    );
    git(publisherRepository, ['config', 'core.autocrlf', 'false']);
    const commitA = commit(
      publisherRepository,
      'Publish pinned Moonfall experiment A',
      '2026-08-31T00:00:00Z',
    );
    const payloadA = gitBytes(publisherRepository, ['show', `${commitA}:${PAYLOAD_PATH}`]);
    const digestA = sha256(payloadA);
    assert.equal(payloadA.compare(sourceBefore), 0, 'published A must equal Moonfall source');

    const trackedA = git(publisherRepository, [
      'ls-tree',
      '-r',
      '--name-only',
      commitA,
    ])
      .trim()
      .split('\n');
    assert.deepEqual(trackedA, [
      'LICENSE-APACHE',
      'LICENSE-MIT',
      'README.md',
      'SOURCE-LICENSE.md',
      PAYLOAD_PATH,
    ]);
    for (const forbidden of [
      '.env',
      '.tachiko/cache/session.json',
      'private/history/notes.md',
      'local/absolute-paths.txt',
      'share-preview.txt',
    ]) {
      assert.ok(!trackedA.includes(forbidden), `${forbidden} must not enter published Git tree`);
    }

    const tupleA = Object.freeze({
      sourceReference: SOURCE_REFERENCE,
      repository: publisherRepository,
      commitSha: commitA,
      payloadPath: PAYLOAD_PATH,
      payloadDigest: digestA,
    });
    const resolvedA = resolveTuple(tupleA);
    assert.equal(resolvedA.available, true);
    assert.equal(resolvedA.actualDigest, digestA);
    assert.equal(resolvedA.bytes.compare(sourceBefore), 0);

    // Inspection happens from the exact commit before the consumer asset area
    // exists. The moving branch is mentioned only as a discovery hint.
    assert.equal(await pathExists(vendoredDirectory), false);
    await mkdir(inspectionDirectory, { recursive: true });
    const pinnedAPath = join(inspectionDirectory, 'pinned-A.ro');
    await writeFile(pinnedAPath, resolvedA.bytes);
    const descriptorA = git(
      publisherRepository,
      ['show', `${commitA}:README.md`],
    );
    validateDeterministically(pinnedAPath, 'pinned A');
    const inspectionSummary = `# PRE-USE INSPECTION — ISSUE #185 EXPERIMENT

artifact class: whole-project/template-style game-balance example
source repository reference: ${SOURCE_REFERENCE}
exact Git commit SHA: ${commitA}
payload path: ${PAYLOAD_PATH}
payload SHA-256: ${digestA}
discovery hint: main (moving, non-authoritative, never consumed directly)
contributes: schemas characters/economy/items/weapons; entities Alric/Iron Sword/Shop/Tempered Blade; DPS/economy formulas; typed references
license/source: upstream Tachiko Work; Apache-2.0 OR MIT declaration; future asset applicability unresolved
compatibility: current repository tachiko validate/set/diff/merge behavior only
required semantic capabilities: read/validate, explicit set, diff, merge; requirements, not grants
executable/external effects: none
reusable-asset dependencies: none
validation evidence: pinned payload validated twice with byte-identical CLI output
trust boundary: Git hosting/listing implies no verification, support, trust, signing, approval, capability grant, or semantic authority
relationship choice: Use/reference unavailable; Copy/Vendor and Fork/Remix are explicit independent operations
`;
    const inspectionPath = join(inspectionDirectory, 'pre-use-inspection.txt');
    await writeFile(inspectionPath, inspectionSummary);
    for (const requiredFact of [
      'whole-project/template-style',
      commitA,
      digestA,
      'schemas characters/economy/items/weapons',
      'entities Alric/Iron Sword/Shop/Tempered Blade',
      'DPS/economy formulas; typed references',
      'future asset applicability unresolved',
      'requirements, not grants',
      'executable/external effects: none',
      'reusable-asset dependencies: none',
      'implies no verification, support, trust',
      'Use/reference unavailable',
    ]) {
      assertContains(inspectionSummary, requiredFact, 'pre-use inspection');
    }
    assertContains(descriptorA, 'not a Tachiko', 'pinned descriptor');
    assert.equal(await pathExists(vendoredDirectory), false);

    // Copy/Vendor is an explicit independent copy. Origin facts remain a
    // sidecar human record, not hidden Tachiko semantic state.
    await mkdir(vendoredDirectory, { recursive: true });
    const vendoredAPath = join(vendoredDirectory, 'game-balance.ro');
    await writeFile(vendoredAPath, resolvedA.bytes);
    await writeFile(
      join(vendoredDirectory, 'ORIGIN.md'),
      `# Experiment-only origin record

Copy/Vendor of ${tupleA.sourceReference} at exact commit ${commitA}, path
${PAYLOAD_PATH}, payload SHA-256 ${digestA}. This copy has no live update
relationship and carries no inherited trust, support, approval, or capability.
`,
    );
    assert.equal(sha256(await readFile(vendoredAPath)), digestA);
    validateDeterministically(vendoredAPath, 'vendored A');

    // Fork/Remix is a separate Git repository with human-readable ancestry.
    await mkdir(join(remixRepository, 'asset'), { recursive: true });
    run(
      'git',
      [
        'init',
        '--quiet',
        '--initial-branch=main',
        '--object-format=sha1',
        remixRepository,
      ],
      { env: CLEAN_PROCESS_ENVIRONMENT },
    );
    git(remixRepository, ['config', 'core.autocrlf', 'false']);
    await writeFile(join(remixRepository, PAYLOAD_PATH), resolvedA.bytes);
    await writeFile(
      join(remixRepository, 'ANCESTRY.md'),
      `# Experiment-only remix ancestry

Derived from ${tupleA.sourceReference}, exact commit ${commitA}, payload path
${PAYLOAD_PATH}, payload SHA-256 ${digestA}. This record is transport provenance
only and grants no trust, support, compatibility, approval, or authority.
`,
    );
    commit(
      remixRepository,
      'Copy pinned Moonfall A for remix',
      '2026-08-31T00:01:00Z',
    );

    const localRemixPath = join(remixRepository, 'asset', 'local-remix.ro');
    const localSetOutput = tachiko([
      'set',
      join(remixRepository, PAYLOAD_PATH),
      'iron_sword.damage',
      '45',
      '--output',
      localRemixPath,
    ]);
    assertContains(localSetOutput, 'affected dps: 40 -> 50', 'local Remix change');
    validateDeterministically(localRemixPath, 'local Remix');
    commit(
      remixRepository,
      'Remix Iron Sword damage through Tachiko CLI',
      '2026-08-31T00:02:00Z',
    );
    const localRemixBeforeUpdate = await readFile(localRemixPath);
    const localRemixDigest = sha256(localRemixBeforeUpdate);

    // An incomplete moving update hint cannot surface a consumable update.
    const missingHintPath = join(inspectionDirectory, 'rejected-missing-B.txt');
    const missingB = await surfaceDiscoveryHint(
      {
        ...tupleA,
        commitSha: '0000000000000000000000000000000000000000',
      },
      missingHintPath,
    );
    assert.equal(missingB.available, false);
    assert.equal(missingB.reason, 'immutable_payload_unresolvable');
    assert.equal(await pathExists(missingHintPath), false);
    assert.equal(sha256(await readFile(pinnedAPath)), digestA);
    assert.equal(sha256(await readFile(localRemixPath)), localRemixDigest);

    // Publisher creates B with a disjoint semantic change using current CLI
    // authority, but keeps it off the moving discovery branch until its exact
    // immutable tuple has resolved and matched.
    git(publisherRepository, ['switch', '--quiet', '-c', 'publication-B']);
    const publisherBOutput = join(probeRoot, 'publisher-B.ro');
    const upstreamSetOutput = tachiko([
      'set',
      join(publisherRepository, PAYLOAD_PATH),
      'iron_sword.attack_interval',
      '0.8',
      '--output',
      publisherBOutput,
    ]);
    assertContains(upstreamSetOutput, 'affected dps: 40 -> 45', 'upstream B change');
    await cp(publisherBOutput, join(publisherRepository, PAYLOAD_PATH));
    const commitB = commit(
      publisherRepository,
      'Publish Moonfall experiment B',
      '2026-08-31T00:03:00Z',
    );
    assert.notEqual(commitB, commitA);
    const payloadB = gitBytes(publisherRepository, ['show', `${commitB}:${PAYLOAD_PATH}`]);
    const digestB = sha256(payloadB);
    assert.notEqual(digestB, digestA);
    const tupleB = Object.freeze({
      ...tupleA,
      commitSha: commitB,
      payloadDigest: digestB,
    });

    const mismatchedHintPath = join(inspectionDirectory, 'rejected-mismatched-B.txt');
    const mismatchedB = await surfaceDiscoveryHint(
      { ...tupleB, payloadDigest: digestA },
      mismatchedHintPath,
    );
    assert.equal(mismatchedB.available, false);
    assert.equal(mismatchedB.reason, 'payload_digest_mismatch');
    assert.equal(await pathExists(mismatchedHintPath), false);
    assert.equal(
      git(publisherRepository, ['rev-parse', 'main']).trim(),
      commitA,
      'moving discovery branch must remain at A while B identity is rejected',
    );
    assert.equal(sha256(await readFile(pinnedAPath)), digestA);
    assert.equal(sha256(await readFile(localRemixPath)), localRemixDigest);

    // Verify the immutable payload first, then advance the moving discovery
    // pointer, then surface the non-authoritative hint.
    const verifiedB = resolveTuple(tupleB);
    assert.equal(verifiedB.available, true);
    assert.equal(verifiedB.actualDigest, digestB);
    git(publisherRepository, ['branch', '--force', 'main', commitB]);
    assert.equal(git(publisherRepository, ['rev-parse', 'main']).trim(), commitB);

    const acceptedHintPath = join(inspectionDirectory, 'available-B.txt');
    const resolvedB = await surfaceDiscoveryHint(tupleB, acceptedHintPath);
    assert.equal(resolvedB.available, true);
    assert.equal(resolvedB.actualDigest, digestB);
    assert.equal(await pathExists(acceptedHintPath), true);
    const acceptedHint = await readFile(acceptedHintPath, 'utf8');
    assertContains(acceptedHint, commitB, 'accepted B discovery hint');
    assertContains(acceptedHint, digestB, 'accepted B discovery hint');

    const pinnedAAfterB = gitBytes(publisherRepository, [
      'show',
      `${commitA}:${PAYLOAD_PATH}`,
    ]);
    assert.equal(pinnedAAfterB.compare(payloadA), 0, 'A must remain reproducible after B');
    assert.equal(sha256(pinnedAAfterB), digestA);

    const upstreamBPath = join(inspectionDirectory, 'upstream-B.ro');
    await writeFile(upstreamBPath, resolvedB.bytes);
    validateDeterministically(upstreamBPath, 'upstream B');

    // Manual inspection produces semantic diffs before applying anything.
    const aToBDiff = tachiko(['diff', pinnedAPath, upstreamBPath]);
    assertContains(aToBDiff, 'attack_interval: 0.9 -> 0.8', 'A to B diff');
    assertContains(aToBDiff, 'affected dps: 40 -> 45', 'A to B diff');

    const remixToBDiff = tachiko(['diff', localRemixPath, upstreamBPath]);
    assertContains(remixToBDiff, 'damage: 45 -> 36', 'local Remix to B divergence');
    assertContains(
      remixToBDiff,
      'attack_interval: 0.9 -> 0.8',
      'local Remix to B divergence',
    );
    assert.equal(sha256(await readFile(localRemixPath)), localRemixDigest);

    // Merge writes only a new candidate, preserving the local Remix byte-for-byte.
    const mergedCandidatePath = join(consumerWorkspace, 'merge-candidate.ro');
    const mergeOutput = tachiko([
      'merge',
      pinnedAPath,
      localRemixPath,
      upstreamBPath,
      '--output',
      mergedCandidatePath,
    ]);
    assertContains(mergeOutput, 'wrote', 'three-way merge');
    validateDeterministically(mergedCandidatePath, 'merged candidate');
    const mergedDiff = tachiko(['diff', pinnedAPath, mergedCandidatePath]);
    assertContains(mergedDiff, 'damage: 36 -> 45', 'merged candidate');
    assertContains(mergedDiff, 'attack_interval: 0.9 -> 0.8', 'merged candidate');
    assertContains(mergedDiff, 'affected dps: 40 -> 56.25', 'merged candidate');
    const mergedCalculation = JSON.parse(tachiko(['calculate', mergedCandidatePath]));
    assert.equal(mergedCalculation['iron_sword.damage'], 45);
    assert.equal(mergedCalculation['iron_sword.attack_interval'], 0.8);
    assert.equal(mergedCalculation['iron_sword.dps'], 56.25);
    assert.equal(sha256(await readFile(localRemixPath)), localRemixDigest);

    assert.equal(git(publisherRepository, ['status', '--porcelain']), '');
    assert.equal(git(remixRepository, ['status', '--porcelain']), '');
    git(publisherRepository, ['diff', '--check', `${commitA}..${commitB}`]);
    git(remixRepository, ['diff', '--check', 'HEAD~1..HEAD']);

    const sourceAfter = await readFile(MOONFALL_SOURCE);
    const sourceStatusAfter = git(REPOSITORY_ROOT, [
      'status',
      '--porcelain',
      '--',
      'examples/game-balance/game-balance.ro',
    ]);
    assert.equal(sourceAfter.compare(sourceBefore), 0, 'checked-in Moonfall bytes must not change');
    assert.equal(
      sourceStatusAfter,
      sourceStatusBefore,
      'checked-in Moonfall working-tree status must not change',
    );

    process.stdout.write(
      [
        'Issue #185 E9 probe: PASS',
        `publisher commit A: ${commitA}`,
        `publisher payload A SHA-256: ${digestA}`,
        `local Remix SHA-256: ${localRemixDigest}`,
        `publisher commit B: ${commitB}`,
        `publisher payload B SHA-256: ${digestB}`,
        'allowlist/sentinels: PASS',
        'pre-use exact-commit inspection: PASS',
        'Copy/Vendor + Fork/Remix: PASS',
        'missing/mismatched B withheld: PASS',
        'A reproducible after B: PASS',
        'manual diff + three-way merge + deterministic validation: PASS',
        'local Remix preserved byte-for-byte: PASS',
        'checked-in Moonfall preserved byte-for-byte: PASS',
        '',
      ].join('\n'),
    );
  } finally {
    if (keepTemporaryArea) {
      process.stderr.write(`kept temporary E9 area: ${probeRoot}\n`);
    } else {
      await rm(probeRoot, { recursive: true, force: true });
    }
  }
}

await main();
