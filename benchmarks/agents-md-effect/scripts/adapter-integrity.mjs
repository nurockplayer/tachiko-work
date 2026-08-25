import {createHash} from "node:crypto";
import {
  chmod, lstat, mkdir, readFile, readdir, readlink, realpath, rm, writeFile,
} from "node:fs/promises";
import {basename, dirname, isAbsolute, relative, resolve} from "node:path";
import {fileURLToPath} from "node:url";
import {denyReadProfile, runNetworkSandboxed} from "./network-sandbox.mjs";

const SHA256 = /^[0-9a-f]{64}$/;
const SHA1 = /^[0-9a-f]{40}$/;

function fail(message) { throw new Error(message); }
function sha256(bytes) { return createHash("sha256").update(bytes).digest("hex"); }
function isInside(candidate, parent) {
  const path = relative(parent, candidate);
  return path === "" || (!path.startsWith("..") && !isAbsolute(path));
}

async function regular(path, label) {
  if (!isAbsolute(path)) fail(`${label} must be absolute`);
  const metadata = await lstat(path);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    fail(`${label} must be a non-symlink regular file`);
  }
  return realpath(path);
}

async function identity(path) {
  const bytes = await readFile(path);
  return {path, bytes: bytes.length, sha256: sha256(bytes)};
}

async function contentTreeIdentity(root) {
  const canonical = await realpath(root);
  const entries = [];
  let readOnly = true;
  async function walk(directory, prefix = "") {
    const children = await readdir(directory);
    children.sort((left, right) => Buffer.from(left).compare(Buffer.from(right)));
    for (const name of children) {
      const path = resolve(directory, name);
      const relativePath = prefix ? `${prefix}/${name}` : name;
      const metadata = await lstat(path);
      readOnly &&= (metadata.mode & 0o222) === 0;
      if (metadata.isDirectory()) {
        entries.push({path: relativePath, type: "directory"});
        await walk(path, relativePath);
      } else if (metadata.isFile()) {
        const bytes = await readFile(path);
        entries.push({path: relativePath, type: "file", bytes: bytes.length,
          sha256: sha256(bytes)});
      } else if (metadata.isSymbolicLink()) {
        entries.push({path: relativePath, type: "symlink", target: await readlink(path)});
      } else fail(`trusted content tree contains an unsupported node: ${relativePath}`);
    }
  }
  const rootMetadata = await lstat(canonical);
  readOnly &&= (rootMetadata.mode & 0o222) === 0;
  await walk(canonical);
  return {
    path: canonical,
    entries: entries.length,
    manifest_sha256: sha256(`${JSON.stringify(entries)}\n`),
    read_only: readOnly,
  };
}

function exactKeys(value, expected, label) {
  if (!value || typeof value !== "object" || Array.isArray(value) ||
      JSON.stringify(Object.keys(value).sort()) !== JSON.stringify([...expected].sort())) {
    fail(`${label} keys mismatch`);
  }
}

function canonicalBytes(value) {
  return Buffer.from(`${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function containsIdentity(entries, expected) {
  return entries?.some((entry) =>
    entry.path === expected.path && entry.bytes === expected.bytes &&
    entry.sha256 === expected.sha256);
}

async function verifyControllerBuiltTw09Probe({
  context,
  controllerContextPath,
  candidateRoot,
  candidateManifestPath,
  probeSourcePath,
  configIdentity,
  integrityIdentity,
  builtProbePath,
  expectedBuiltProbeSha256,
  probeBuildReceiptPath,
  expectedProbeBuildReceiptSha256,
  adapterBuildStageReceiptPath,
  expectedAdapterBuildStageReceiptSha256,
  cargoPath,
  cargoSha256,
  rustcPath,
  rustcSha256,
}) {
  if (![expectedBuiltProbeSha256, expectedProbeBuildReceiptSha256,
    expectedAdapterBuildStageReceiptSha256].every((value) => SHA256.test(value ?? ""))) {
    fail("controller-built TW-09 probe identities are incomplete");
  }
  const [candidate, candidateManifest, probeSource, probe, buildReceiptPath,
    buildStagePath, cargo, rustc, contextPath] = await Promise.all([
      realpath(candidateRoot),
      regular(candidateManifestPath, "trusted captured candidate raw manifest"),
      regular(probeSourcePath, "formal TW-09 reviewer probe source"),
      regular(builtProbePath, "controller-built TW-09 probe"),
      regular(probeBuildReceiptPath, "controller TW-09 probe build receipt"),
      regular(adapterBuildStageReceiptPath, "controller adapter-build stage receipt"),
      regular(cargoPath, "trusted Cargo for formal TW-09 probe build"),
      regular(rustcPath, "trusted rustc for formal TW-09 probe build"),
      regular(controllerContextPath, "controller evidence context"),
    ]);
  const [manifestIdentity, sourceIdentity, probeIdentity, buildReceiptIdentity,
    buildStageIdentity, cargoIdentity, rustcIdentity] = await Promise.all([
      identity(candidateManifest), identity(probeSource), identity(probe),
      identity(buildReceiptPath), identity(buildStagePath), identity(cargo), identity(rustc),
    ]);
  if (probeIdentity.sha256 !== expectedBuiltProbeSha256 ||
      buildReceiptIdentity.sha256 !== expectedProbeBuildReceiptSha256 ||
      buildStageIdentity.sha256 !== expectedAdapterBuildStageReceiptSha256 ||
      cargoIdentity.sha256 !== cargoSha256 || rustcIdentity.sha256 !== rustcSha256 ||
      manifestIdentity.sha256 !== context.raw_tree_digest_sha256) {
    fail("controller-built TW-09 probe identity mismatch");
  }
  const artifactRoot = dirname(contextPath);
  if (dirname(buildStagePath) !== resolve(artifactRoot, "stage-receipts") ||
      !isInside(probe, artifactRoot) || !isInside(buildReceiptPath, artifactRoot)) {
    fail("controller-built TW-09 evidence escaped the authorized attempt artifacts");
  }

  const buildReceipt = JSON.parse((await readFile(buildReceiptPath)).toString("utf8"));
  const helperIdentity = await identity(fileURLToPath(import.meta.url));
  if (buildReceipt.schema !== "tachiko-controller-tw09-probe-build-v1" ||
      buildReceipt.classification !== "formal_controller_build" ||
      buildReceipt.case_id !== "TW-09" ||
      buildReceipt.capture_receipt_sha256 !== context.capture_receipt_sha256 ||
      buildReceipt.candidate_tree !== context.candidate_tree ||
      buildReceipt.raw_tree_digest_sha256 !== context.raw_tree_digest_sha256 ||
      JSON.stringify(buildReceipt.candidate_raw_manifest) !== JSON.stringify(manifestIdentity) ||
      JSON.stringify(buildReceipt.reviewer_probe_source) !== JSON.stringify(sourceIdentity) ||
      JSON.stringify(buildReceipt.tools?.cargo) !== JSON.stringify(cargoIdentity) ||
      JSON.stringify(buildReceipt.tools?.rustc) !== JSON.stringify(rustcIdentity) ||
      JSON.stringify(buildReceipt.helper) !== JSON.stringify(helperIdentity) ||
      JSON.stringify(buildReceipt.probe) !== JSON.stringify(probeIdentity) ||
      buildReceipt.kernel_sandbox?.deny_network !== true ||
      buildReceipt.kernel_sandbox?.deny_unlisted_reads !== true ||
      buildReceipt.kernel_sandbox?.deny_unlisted_writes !== true ||
      buildReceipt.commands?.generate_lockfile?.process_supervision?.timed_out !== false ||
      buildReceipt.commands?.generate_lockfile?.process_supervision
        ?.process_group_extinct_before_capture !== true ||
      buildReceipt.commands?.build?.process_supervision?.timed_out !== false ||
      buildReceipt.commands?.build?.process_supervision?.process_group_extinct_before_capture !== true ||
      buildReceipt.commands?.metadata?.process_supervision?.timed_out !== false ||
      buildReceipt.commands?.metadata?.process_supervision?.process_group_extinct_before_capture !== true ||
      buildReceipt.build_closure?.candidate_and_trusted_inputs_postchecked_unchanged !== true ||
      buildReceipt.cargo_home?.unchanged !== true ||
      buildReceipt.cargo_home?.fully_read_only_for_formal_build !== true ||
      buildReceipt.cargo_home?.before?.manifest_sha256 !==
        buildReceipt.cargo_home?.after?.manifest_sha256 ||
      buildReceipt.cargo_home?.before?.entries !== buildReceipt.cargo_home?.after?.entries ||
      !Array.isArray(buildReceipt.build_closure?.candidate_packages) ||
      buildReceipt.build_closure.candidate_packages.length === 0 ||
      buildReceipt.kernel_sandbox?.trusted_inputs_read_only_during_candidate_compilation !== true) {
    fail("controller-built TW-09 probe build receipt contract mismatch");
  }

  const rawManifest = JSON.parse((await readFile(candidateManifest)).toString("utf8"));
  exactKeys(rawManifest, ["version", "entries"], "captured candidate raw manifest");
  const expectedSourceEntries = rawManifest.entries?.map(({path, mode, bytes, sha256: hash}) => ({
    path, mode, bytes, sha256: hash,
  }));
  if (rawManifest.version !== 1 || !Array.isArray(rawManifest.entries) ||
      JSON.stringify(buildReceipt.candidate_source_entries) !== JSON.stringify(expectedSourceEntries)) {
    fail("controller-built TW-09 probe does not bind the complete captured source closure");
  }
  for (const entry of rawManifest.entries) {
    for (const [label, root] of [
      ["captured candidate", candidate],
      ["controller build mirror", resolve(dirname(buildReceiptPath), "candidate")],
    ]) {
      const source = resolve(root, entry.path);
      if (!isInside(source, root) || await regular(`${source}`, `${label} source`) !== source) {
        fail(`${label} source escaped its root`);
      }
      const bytes = await readFile(source);
      if (entry.type !== "regular" || bytes.length !== entry.bytes ||
          sha256(bytes) !== entry.sha256) {
        fail(`${label} source differs from its trusted complete raw manifest`);
      }
    }
  }
  const buildRoot = dirname(buildReceiptPath);
  const copiedProbe = await identity(await regular(
    resolve(buildRoot, "probe-package/probe.rs"),
    "controller build reviewer probe copy",
  ));
  const generatedManifest = await identity(await regular(
    resolve(buildRoot, "probe-package/Cargo.toml"),
    "controller build manifest",
  ));
  const cargoLock = await identity(await regular(
    resolve(buildRoot, "probe-package/Cargo.lock"),
    "controller build Cargo.lock",
  ));
  const expectedWritableRoots = [resolve(buildRoot, "target"), resolve(buildRoot, "tmp")];
  if (JSON.stringify(buildReceipt.reviewer_probe_build_copy) !== JSON.stringify(copiedProbe) ||
      JSON.stringify(buildReceipt.manifest) !== JSON.stringify(generatedManifest) ||
      JSON.stringify(buildReceipt.cargo_lock) !== JSON.stringify(cargoLock) ||
      JSON.stringify(buildReceipt.kernel_sandbox.build_writable_roots) !==
        JSON.stringify(expectedWritableRoots)) {
    fail("controller-built TW-09 trusted build inputs or write closure changed");
  }

  const stage = JSON.parse((await readFile(buildStagePath)).toString("utf8"));
  for (const key of ["protocol_id", "phase", "classification", "formal_result_eligible",
    "wave_id", "run_id", "attempt_id", "candidate_id", "case_id"]) {
    if (stage[key] !== context[key]) fail(`controller adapter-build stage ${key} mismatch`);
  }
  if (stage.schema !== "tachiko-controller-stage-receipt-v1" ||
      stage.stage !== "formal_adapter_build" ||
      stage.payload_sha256 !== sha256(canonicalBytes(stage.payload)) ||
      stage.formal_authorization?.sha256 !== context.formal_authorization_sha256 ||
      stage.payload?.probe_sha256 !== probeIdentity.sha256 ||
      stage.payload?.probe_build_receipt_sha256 !== buildReceiptIdentity.sha256 ||
      stage.payload?.cargo_home_manifest_sha256 !==
        buildReceipt.cargo_home.before.manifest_sha256 ||
      !containsIdentity(stage.inputs, configIdentity) ||
      !containsIdentity(stage.inputs, integrityIdentity) ||
      !containsIdentity(stage.inputs, sourceIdentity) ||
      !containsIdentity(stage.inputs, manifestIdentity) ||
      !containsIdentity(stage.outputs, probeIdentity) ||
      !containsIdentity(stage.outputs, buildReceiptIdentity)) {
    fail("controller adapter-build stage does not bind the sealed TW-09 probe closure");
  }
  return {
    probe: probeIdentity,
    candidate_manifest: manifestIdentity,
    probe_source: sourceIdentity,
    probe_build_receipt: {
      ...buildReceiptIdentity,
      cargo_home_manifest_sha256: buildReceipt.cargo_home.before.manifest_sha256,
    },
    adapter_build_stage_receipt: buildStageIdentity,
  };
}

function processEvidence(execution, timeoutMilliseconds) {
  return {
    exit_code: execution.exit_code,
    signal: execution.signal,
    spawn_error: execution.spawn_error,
    timed_out: execution.timed_out,
    deadline_seconds: timeoutMilliseconds / 1000,
    termination_grace_seconds: execution.termination_grace_seconds,
    termination_signal_sent: execution.termination_signal_sent,
    kill_signal_sent: execution.kill_signal_sent,
    process_group_extinct_before_capture: execution.process_group_extinct_before_capture,
    process_containment: execution.process_containment,
    stdout_sha256: sha256(execution.stdout),
    stderr_sha256: sha256(execution.stderr),
  };
}

async function runBuilderCommand({
  executable, args, cwd, environment, profile, timeoutMilliseconds, terminationGraceMilliseconds,
}) {
  const execution = await runNetworkSandboxed({
    executable,
    args,
    cwd,
    environment,
    timeoutMilliseconds,
    terminationGraceMilliseconds,
    maxOutputBytes: 128 * 1024 * 1024,
    profile,
  });
  if (execution.exit_code !== 0 || execution.spawn_error || execution.timed_out ||
      !execution.process_group_extinct_before_capture) {
    fail(`sealed TW-09 probe build command failed: ${execution.stderr.toString("utf8")}`);
  }
  return execution;
}

async function requireAbsent(path, label) {
  try {
    await lstat(path);
    fail(`${label} is forbidden`);
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }
}

async function assertBuildInputsUnchanged({
  candidate,
  copiedCandidate,
  sourceEntries,
  trustedInputs,
}) {
  for (const entry of sourceEntries) {
    for (const root of [candidate, copiedCandidate]) {
      const path = resolve(root, entry.path);
      if (await regular(path, "sealed TW-09 candidate source postcheck") !== path) {
        fail("sealed TW-09 candidate source postcheck traversed a symlink");
      }
      const bytes = await readFile(path);
      if (bytes.length !== entry.bytes || sha256(bytes) !== entry.sha256) {
        fail("sealed TW-09 candidate source changed during the trusted build");
      }
    }
  }
  for (const expected of trustedInputs) {
    if (JSON.stringify(await identity(expected.path)) !== JSON.stringify(expected)) {
      fail("sealed TW-09 reviewer or controller build input changed during the trusted build");
    }
  }
}

function inspectCargoMetadata(metadata, copiedCandidate, packageRoot) {
  if (!metadata || typeof metadata !== "object" || !Array.isArray(metadata.packages)) {
    fail("trusted Cargo metadata did not describe the TW-09 build closure");
  }
  const candidatePackages = [];
  const trustedRegistryProcMacros = [];
  for (const pkg of metadata.packages) {
    const manifestPath = resolve(pkg.manifest_path ?? "");
    const candidatePackage = isInside(manifestPath, copiedCandidate);
    const controllerPackage = isInside(manifestPath, packageRoot);
    if (!candidatePackage && !controllerPackage &&
        (typeof pkg.source !== "string" || !pkg.source.startsWith("registry+"))) {
      fail("TW-09 build closure contains an unsealed non-registry package");
    }
    const targets = Array.isArray(pkg.targets) ? pkg.targets : [];
    const compileTimeTargets = targets.filter((target) =>
      target.kind?.includes("custom-build") || target.kind?.includes("proc-macro") ||
      target.crate_types?.includes("proc-macro"));
    if (candidatePackage && compileTimeTargets.length > 0) {
      fail("candidate compile-time execution via a build script or proc-macro target is forbidden");
    }
    if (candidatePackage) {
      candidatePackages.push({
        id: pkg.id,
        manifest_path: manifestPath,
        targets: targets.map((target) => ({
          name: target.name,
          kind: target.kind,
          crate_types: target.crate_types,
          src_path: target.src_path,
        })),
      });
    } else if (compileTimeTargets.some((target) =>
      target.kind?.includes("proc-macro") || target.crate_types?.includes("proc-macro"))) {
      if (typeof pkg.source !== "string" || !pkg.source.startsWith("registry+")) {
        fail("TW-09 build contains an unsealed non-registry proc-macro");
      }
      trustedRegistryProcMacros.push({id: pkg.id, source: pkg.source, manifest_path: manifestPath});
    }
  }
  if (candidatePackages.length === 0) {
    fail("trusted Cargo metadata did not exercise a captured candidate package");
  }
  return {candidate_packages: candidatePackages, trusted_registry_proc_macros: trustedRegistryProcMacros};
}

export async function buildFormalTw09CandidateProbe({
  candidateRoot,
  candidateTree,
  captureReceiptSha256,
  rawTreeDigestSha256,
  candidateManifestPath,
  probeSourcePath,
  expectedProbeSha256,
  buildRoot,
  cargoPath,
  cargoSha256,
  rustcPath,
  rustcSha256,
  environment = process.env,
  timeoutMilliseconds = 1_800_000,
  terminationGraceMilliseconds = 10_000,
  constructionPreview = false,
}) {
  if (!SHA1.test(candidateTree ?? "") || !SHA256.test(captureReceiptSha256 ?? "") ||
      !SHA256.test(rawTreeDigestSha256 ?? "") ||
      (constructionPreview
        ? expectedProbeSha256 !== undefined
        : !SHA256.test(expectedProbeSha256 ?? ""))) {
    fail("sealed TW-09 probe build bindings are invalid");
  }
  const candidate = await realpath(candidateRoot);
  const candidateManifest = await regular(
    candidateManifestPath,
    "trusted captured candidate raw manifest",
  );
  const probeSource = await regular(probeSourcePath, "formal TW-09 reviewer probe source");
  const cargo = await regular(cargoPath, "trusted Cargo for formal TW-09 probe build");
  const rustc = await regular(rustcPath, "trusted rustc for formal TW-09 probe build");
  const rustLinker = await regular(
    resolve(dirname(dirname(rustc)), "lib/rustlib/aarch64-apple-darwin/bin/rust-lld"),
    "Rust-toolchain linker for formal TW-09 probe build",
  );
  const [candidateManifestIdentity, probeSourceIdentity, cargoIdentity, rustcIdentity,
    rustLinkerIdentity] =
    await Promise.all([
      identity(candidateManifest), identity(probeSource), identity(cargo), identity(rustc),
      identity(rustLinker),
  ]);
  if (cargoIdentity.sha256 !== cargoSha256 || rustcIdentity.sha256 !== rustcSha256) {
    fail("sealed TW-09 probe build tool identity mismatch");
  }
  if (candidateManifestIdentity.sha256 !== rawTreeDigestSha256) {
    fail("sealed TW-09 probe build raw candidate manifest identity mismatch");
  }
  const requestedBuildRoot = resolve(buildRoot);
  try {
    await lstat(requestedBuildRoot);
    fail("sealed TW-09 probe build root must be fresh and absent");
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }
  for (const input of [candidate, candidateManifest, probeSource, cargo, rustc]) {
    if (isInside(requestedBuildRoot, input) || isInside(input, requestedBuildRoot)) {
      fail("sealed TW-09 probe build root must be disjoint from every trusted input");
    }
  }
  await mkdir(requestedBuildRoot, {recursive: false, mode: 0o700});
  const copiedCandidate = resolve(requestedBuildRoot, "candidate");
  const packageRoot = resolve(requestedBuildRoot, "probe-package");
  const targetRoot = resolve(requestedBuildRoot, "target");
  const tmpRoot = resolve(requestedBuildRoot, "tmp");
  await Promise.all([
    mkdir(copiedCandidate, {mode: 0o700}), mkdir(packageRoot, {mode: 0o700}),
    mkdir(targetRoot, {mode: 0o700}), mkdir(tmpRoot, {mode: 0o700}),
  ]);
  const cargoHome = resolve(environment.CARGO_HOME ?? resolve(environment.HOME, ".cargo"));
  const sdkRoot = await realpath("/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk");
  const runtimeRoots = [...new Set([
    dirname(dirname(cargo)),
    dirname(dirname(rustc)),
    cargoHome,
    ...(environment.RUSTUP_HOME ? [resolve(environment.RUSTUP_HOME)] : []),
    sdkRoot,
  ])];
  for (const name of ["config", "config.toml"]) {
    await requireAbsent(resolve(cargoHome, name), `Cargo home ${name}`);
  }
  const cargoHomeBefore = await contentTreeIdentity(cargoHome);
  if (!constructionPreview && !cargoHomeBefore.read_only) {
    fail("formal TW-09 Cargo home is not fully read-only");
  }
  let ancestor = packageRoot;
  while (ancestor !== dirname(ancestor)) {
    for (const name of ["config", "config.toml"]) {
      await requireAbsent(resolve(ancestor, ".cargo", name), `ambient Cargo ${name}`);
    }
    ancestor = dirname(ancestor);
  }
  const generateProfile = denyReadProfile([], {
    denyUnlistedReads: true,
    denyUnlistedWrites: true,
    allowReadPaths: [cargo, rustc, rustLinker],
    allowReadRoots: [requestedBuildRoot, ...runtimeRoots],
    allowWriteRoots: [packageRoot, targetRoot, tmpRoot],
  });
  const buildProfile = denyReadProfile([], {
    denyUnlistedReads: true,
    denyUnlistedWrites: true,
    allowReadPaths: [cargo, rustc, rustLinker],
    allowReadRoots: [requestedBuildRoot, ...runtimeRoots],
    allowWriteRoots: [targetRoot, tmpRoot],
  });
  const buildEnvironment = {...environment};
  for (const key of Object.keys(buildEnvironment)) {
    if (/^(?:RUSTC_WRAPPER|RUSTC_WORKSPACE_WRAPPER|RUSTDOC|RUSTDOCFLAGS|CARGO_BUILD_|CARGO_TARGET_)/
      .test(key)) delete buildEnvironment[key];
  }
  Object.assign(buildEnvironment, {
    CARGO_HOME: cargoHome,
    CARGO_NET_OFFLINE: "true",
    CARGO_TARGET_DIR: targetRoot,
    RUSTC: rustc,
    RUSTFLAGS: `-C linker=${rustLinker} -C linker-flavor=ld64.lld ` +
      `-C link-arg=-syslibroot -C link-arg=${sdkRoot} -C link-arg=-no_uuid ` +
      `--remap-path-prefix=${requestedBuildRoot}=/tachiko/controller-build ` +
      `--remap-path-prefix=${cargoHome}=/tachiko/cargo-home`,
    CARGO_ENCODED_RUSTFLAGS: `-Clinker=${rustLinker}\x1f-Clinker-flavor=ld64.lld\x1f` +
      `-Clink-arg=-syslibroot\x1f-Clink-arg=${sdkRoot}\x1f-Clink-arg=-no_uuid\x1f` +
      `--remap-path-prefix=${requestedBuildRoot}=/tachiko/controller-build\x1f` +
      `--remap-path-prefix=${cargoHome}=/tachiko/cargo-home`,
    SDKROOT: sdkRoot,
    SOURCE_DATE_EPOCH: "946684800",
    TMPDIR: tmpRoot,
    TMP: tmpRoot,
    TEMP: tmpRoot,
  });
  const rawManifestBytes = await readFile(candidateManifest);
  const rawManifest = JSON.parse(rawManifestBytes.toString("utf8"));
  exactKeys(rawManifest, ["version", "entries"], "captured candidate raw manifest");
  if (rawManifest.version !== 1 || !Array.isArray(rawManifest.entries) ||
      rawManifest.entries.length === 0) {
    fail("captured candidate raw manifest is empty or invalid");
  }
  const treeEntries = rawManifest.entries;
  const sourceEntries = [];
  let priorPath = null;
  for (const entry of treeEntries) {
    exactKeys(entry, ["path", "type", "mode", "bytes", "sha256"],
      "captured candidate raw manifest entry");
    if (entry.type !== "regular" || !["100644", "100755"].includes(entry.mode) ||
        typeof entry.path !== "string" || entry.path.length === 0 || isAbsolute(entry.path) ||
        entry.path.split("/").includes("..") || !Number.isSafeInteger(entry.bytes) ||
        entry.bytes < 0 || !SHA256.test(entry.sha256 ?? "") ||
        (priorPath !== null && Buffer.from(priorPath).compare(Buffer.from(entry.path)) >= 0)) {
      fail("captured candidate raw manifest is incomplete, unsorted, or unsafe");
    }
    const pathSegments = entry.path.split("/");
    if (pathSegments.includes(".cargo")) {
      fail("candidate compile-time execution configuration under .cargo is forbidden");
    }
    if (pathSegments.at(-1) === "build.rs") {
      fail("candidate compile-time execution via build.rs or a package build script is forbidden");
    }
    priorPath = entry.path;
    const source = resolve(candidate, entry.path);
    if (!isInside(source, candidate)) fail("captured candidate source escaped its root");
    const sourcePath = await regular(source, "captured candidate source");
    if (sourcePath !== source) fail("captured candidate source may not traverse a symlink");
    const bytes = await readFile(sourcePath);
    if (bytes.length !== entry.bytes || sha256(bytes) !== entry.sha256) {
      fail("captured candidate source differs from its trusted complete raw manifest");
    }
    const destination = resolve(copiedCandidate, entry.path);
    await mkdir(dirname(destination), {recursive: true, mode: 0o700});
    await writeFile(destination, bytes, {mode: entry.mode === "100755" ? 0o500 : 0o400, flag: "wx"});
    sourceEntries.push({
      path: entry.path,
      mode: entry.mode,
      bytes: bytes.length,
      sha256: sha256(bytes),
    });
  }
  const copiedProbeSource = resolve(packageRoot, "probe.rs");
  const probeSourceBytes = await readFile(probeSource);
  await writeFile(copiedProbeSource, probeSourceBytes, {mode: 0o400, flag: "wx"});
  const manifest = resolve(packageRoot, "Cargo.toml");
  const manifestBytes = Buffer.from(
    `[workspace]\n\n[package]\nname = "tachiko-tw09-formal-probe"\nversion = "0.0.0"\n` +
      `edition = "2024"\n\n[[bin]]\nname = "tachiko-tw09-formal-probe"\n` +
      `path = "probe.rs"\n\n[dependencies]\n` +
      `tachiko-semantic-core = { path = ${JSON.stringify(resolve(copiedCandidate, "crates/semantic-core"))} }\n` +
      `serde_json = "1.0"\n`,
  );
  await writeFile(manifest, manifestBytes, {mode: 0o400, flag: "wx"});
  const generateArgs = ["generate-lockfile", "--manifest-path", manifest, "--offline"];
  const metadataArgs = [
    "metadata", "--format-version", "1", "--manifest-path", manifest, "--locked", "--offline",
  ];
  const buildArgs = [
    "build", "--manifest-path", manifest, "--locked", "--release", "--offline",
  ];
  const generated = await runBuilderCommand({
    executable: cargo, args: generateArgs, cwd: packageRoot, environment: buildEnvironment,
    profile: generateProfile, timeoutMilliseconds, terminationGraceMilliseconds,
  });
  const lockPath = await regular(resolve(packageRoot, "Cargo.lock"), "sealed TW-09 Cargo.lock");
  await chmod(lockPath, 0o400);
  const lockIdentity = await identity(lockPath);
  const trustedBuildInputs = await Promise.all([
    copiedProbeSource, manifest, lockPath,
  ].map(identity));
  await assertBuildInputsUnchanged({
    candidate, copiedCandidate, sourceEntries, trustedInputs: trustedBuildInputs,
  });
  const metadataExecution = await runBuilderCommand({
    executable: cargo, args: metadataArgs, cwd: packageRoot, environment: buildEnvironment,
    profile: buildProfile, timeoutMilliseconds, terminationGraceMilliseconds,
  });
  let metadata;
  try {
    metadata = JSON.parse(metadataExecution.stdout.toString("utf8"));
  } catch {
    fail("trusted Cargo metadata output for TW-09 was not valid JSON");
  }
  const buildClosure = inspectCargoMetadata(metadata, copiedCandidate, packageRoot);
  const built = await runBuilderCommand({
    executable: cargo, args: buildArgs, cwd: packageRoot, environment: buildEnvironment,
    profile: buildProfile, timeoutMilliseconds, terminationGraceMilliseconds,
  });
  await assertBuildInputsUnchanged({
    candidate, copiedCandidate, sourceEntries, trustedInputs: trustedBuildInputs,
  });
  const cargoHomeAfter = await contentTreeIdentity(cargoHome);
  if (cargoHomeAfter.manifest_sha256 !== cargoHomeBefore.manifest_sha256 ||
      cargoHomeAfter.entries !== cargoHomeBefore.entries ||
      (!constructionPreview && !cargoHomeAfter.read_only)) {
    fail("trusted TW-09 Cargo home changed during the sealed build");
  }
  const probe = await regular(
    resolve(targetRoot, "release/tachiko-tw09-formal-probe"),
    "sealed controller-built TW-09 probe",
  );
  await chmod(probe, 0o500);
  const probeIdentity = await identity(probe);
  if (!constructionPreview && probeIdentity.sha256 !== expectedProbeSha256) {
    fail("sealed controller-built TW-09 probe does not match the independently reviewed hash");
  }
  const helperPath = fileURLToPath(import.meta.url);
  const receipt = {
    schema: "tachiko-controller-tw09-probe-build-v1",
    classification: constructionPreview ? "construction_preview" : "formal_controller_build",
    case_id: "TW-09",
    capture_receipt_sha256: captureReceiptSha256,
    candidate_tree: candidateTree,
    raw_tree_digest_sha256: rawTreeDigestSha256,
    candidate_raw_manifest: candidateManifestIdentity,
    candidate_source_entries: sourceEntries,
    candidate_source_tree_sha256: sha256(`${JSON.stringify(sourceEntries)}\n`),
    reviewer_probe_source: probeSourceIdentity,
    reviewer_probe_build_copy: await identity(copiedProbeSource),
    manifest: await identity(manifest),
    cargo_lock: lockIdentity,
    build_closure: {
      ...buildClosure,
      metadata_stdout_sha256: sha256(metadataExecution.stdout),
      candidate_and_trusted_inputs_postchecked_unchanged: true,
    },
    tools: {cargo: cargoIdentity, rustc: rustcIdentity, linker: rustLinkerIdentity},
    cargo_home: {
      before: cargoHomeBefore,
      after: cargoHomeAfter,
      unchanged: true,
      fully_read_only_for_formal_build: constructionPreview ? null : true,
    },
    commands: {
      generate_lockfile: {
        argv: [cargo, ...generateArgs],
        process_supervision: processEvidence(generated, timeoutMilliseconds),
      },
      metadata: {
        argv: [cargo, ...metadataArgs],
        process_supervision: processEvidence(metadataExecution, timeoutMilliseconds),
      },
      build: {
        argv: [cargo, ...buildArgs],
        process_supervision: processEvidence(built, timeoutMilliseconds),
      },
    },
    kernel_sandbox: {
      executable: "/usr/bin/sandbox-exec",
      generate_lockfile_profile_sha256: sha256(generateProfile),
      build_profile_sha256: sha256(buildProfile),
      deny_network: true,
      deny_unlisted_reads: true,
      deny_unlisted_writes: true,
      trusted_inputs_read_only_during_candidate_compilation: true,
      build_writable_roots: [targetRoot, tmpRoot],
    },
    helper: await identity(helperPath),
    probe: probeIdentity,
  };
  const receiptPath = resolve(requestedBuildRoot, "probe-build-receipt.json");
  const receiptBytes = canonicalBytes(receipt);
  await writeFile(receiptPath, receiptBytes, {mode: 0o400, flag: "wx"});
  return {
    probe: probeIdentity,
    candidate_manifest: candidateManifestIdentity,
    probe_source: probeSourceIdentity,
    probe_build_receipt: {
      path: receiptPath,
      bytes: receiptBytes.length,
      sha256: sha256(receiptBytes),
      cargo_home_manifest_sha256: cargoHomeBefore.manifest_sha256,
    },
    receipt,
  };
}

export async function materializeFormalAdapterEnvelope({
  stdout,
  outputPath,
  caseId,
  contractSha256,
  sandboxProfileSha256,
  processGroupExtinct,
  adapterPackage,
}) {
  if (processGroupExtinct !== true) {
    fail("formal adapter output may only be materialized after process-group extinction");
  }
  const lines = stdout.split(/\r?\n/).filter((line) => line.length > 0);
  if (lines.length !== 1) fail("formal adapter scaffold must emit exactly one receipt line");
  const receipt = JSON.parse(lines[0]);
  exactKeys(receipt, [
    "scaffold", "case_id", "config_sha256", "probe_sha256", "probe_stdout_sha256",
    "probe_stderr_sha256", "sandbox_profile_sha256", "denied_read_roots_sha256", "envelope",
  ], "formal adapter receipt");
  if (
    receipt.scaffold !== "tachiko-candidate-adapter-v1" ||
    receipt.case_id !== caseId ||
    receipt.config_sha256 !== adapterPackage.config.sha256 ||
    receipt.probe_sha256 !== adapterPackage.probe.sha256 ||
    receipt.sandbox_profile_sha256 !== sandboxProfileSha256
  ) {
    fail("formal adapter receipt binding mismatch");
  }
  const envelopeKeys = caseId === "TW-05"
    ? ["contract_sha256", "adapter", "native", "wasm"]
    : caseId === "TW-09"
      ? ["contract_sha256", "adapter", "observations"]
      : fail("formal adapter envelope is only supported for TW-05 and TW-09");
  exactKeys(receipt.envelope, envelopeKeys, "formal adapter envelope");
  exactKeys(
    receipt.envelope.adapter,
    ["sha256", "behavior_implemented_by_adapter"],
    "formal adapter envelope identity",
  );
  if (
    receipt.envelope.contract_sha256 !== contractSha256 ||
    receipt.envelope.adapter.sha256 !== adapterPackage.scaffold.sha256 ||
    receipt.envelope.adapter.behavior_implemented_by_adapter !== false
  ) {
    fail("formal adapter envelope binding mismatch");
  }
  const parent = await realpath(dirname(resolve(outputPath)));
  const output = resolve(parent, basename(outputPath));
  const bytes = Buffer.from(`${JSON.stringify(receipt.envelope)}\n`);
  await writeFile(output, bytes, {mode: 0o600, flag: "wx"});
  return {
    path: output,
    bytes: bytes.length,
    sha256: sha256(bytes),
    materialized_after_process_group_extinction: true,
    source_stdout_sha256: sha256(stdout),
    receipt_sha256: sha256(lines[0]),
  };
}

export async function validateFormalAdapterPackage({
  adapterPath,
  configPath,
  integrityReceiptPath,
  expectedIntegrityReceiptSha256,
  benchmarkRoot,
  forbiddenRoots = [],
  context,
  candidateRoot,
  candidateManifestPath,
  probeSourcePath,
  buildRoot,
  cargoPath,
  cargoSha256,
  rustcPath,
  rustcSha256,
  environment,
  controllerContextPath,
  builtProbePath,
  expectedBuiltProbeSha256,
  probeBuildReceiptPath,
  expectedProbeBuildReceiptSha256,
  adapterBuildStageReceiptPath,
  expectedAdapterBuildStageReceiptSha256,
}) {
  const expectedPath = await realpath(resolve(
    benchmarkRoot,
    "evaluator/adapters/candidate-adapter.mjs",
  ));
  const adapter = await regular(adapterPath, "formal adapter scaffold");
  if (adapter !== expectedPath) {
    fail("formal adapter must be the sealed qualified candidate adapter scaffold");
  }
  const lockPath = await regular(
    resolve(benchmarkRoot, "evaluator/adapters/candidate-adapter-lock.json"),
    "formal adapter scaffold lock",
  );
  const lockBytes = await readFile(lockPath);
  const lock = JSON.parse(lockBytes.toString("utf8"));
  const adapterBytes = await readFile(adapter);
  if (
    lock.schema !== "tachiko-candidate-adapter-lock-v1" ||
    lock.path !== "evaluator/adapters/candidate-adapter.mjs" ||
    lock.sha256 !== sha256(adapterBytes) ||
    lock.bytes !== adapterBytes.length
  ) {
    fail("sealed qualified candidate adapter scaffold does not match its operational lock");
  }
  if (!configPath) fail("formal adapter config is required");
  const config = await regular(configPath, "formal adapter config");
  const configBytes = await readFile(config);
  const parsed = JSON.parse(configBytes.toString("utf8"));
  if (
    parsed?.schema !== "tachiko-candidate-adapter-v1" ||
    parsed.case_id !== context?.case_id ||
    JSON.stringify(Object.keys(parsed).sort()) !== JSON.stringify(["case_id", "probe", "schema"]) ||
    !parsed.probe ||
    JSON.stringify(Object.keys(parsed.probe).sort()) !==
      JSON.stringify(["arguments", "executable", "sha256"])
  ) {
    fail("formal adapter config contract or case binding mismatch");
  }
  const probeRequested = parsed?.probe?.executable;
  const controllerBuiltTw09 = context?.case_id === "TW-09";
  if (controllerBuiltTw09
    ? probeRequested !== "controller-built:TW-09"
    : typeof probeRequested !== "string" || !isAbsolute(probeRequested)) {
    fail(controllerBuiltTw09
      ? "formal TW-09 adapter config must require the sealed controller-built probe"
      : "formal adapter config must name an absolute probe executable");
  }
  let probe = null;
  let probeBytes = null;
  if (!controllerBuiltTw09) {
    probe = await regular(probeRequested, "formal adapter probe");
    if (probeRequested !== probe) {
      fail("formal adapter probe executable must use its canonical sealed path");
    }
    if (((await lstat(probe)).mode & 0o111) === 0) {
      fail("formal adapter probe must be executable");
    }
    probeBytes = await readFile(probe);
  }
  if (!Array.isArray(parsed.probe.arguments) || parsed.probe.arguments.length === 0 ||
      parsed.probe.arguments.some((argument) =>
        typeof argument !== "string" || /<(?:contract|output|trusted-[^>]+)>/i.test(argument))) {
    fail("formal adapter probe arguments contain a forbidden contract or expected-value token");
  }
  if (JSON.stringify(parsed.probe.arguments) !== JSON.stringify(["<candidate-root>"])) {
    fail(
      "formal adapter probe input closure is unbound: a self-contained executable " +
        "may receive only the captured candidate root",
    );
  }
  for (const root of forbiddenRoots) {
    const canonicalRoot = await realpath(root);
    if (isInside(config, canonicalRoot)) {
      fail("formal adapter config is inside a forbidden root and is not disjoint");
    }
    if (probe && isInside(probe, canonicalRoot)) {
      fail("formal adapter probe is inside a forbidden root and is not disjoint");
    }
    for (const argument of parsed.probe.arguments) {
      if (isAbsolute(argument) && isInside(resolve(argument), canonicalRoot)) {
        fail("formal adapter probe arguments expose a forbidden expected-value root");
      }
    }
  }
  if (!integrityReceiptPath || !expectedIntegrityReceiptSha256) {
    fail("formal adapter integrity receipt is required");
  }
  if (!SHA256.test(expectedIntegrityReceiptSha256)) {
    fail("formal adapter integrity receipt expected SHA-256 is invalid");
  }
  const integrityReceipt = await regular(
    integrityReceiptPath,
    "formal adapter integrity receipt",
  );
  for (const root of forbiddenRoots) {
    const canonicalRoot = await realpath(root);
    if (isInside(integrityReceipt, canonicalRoot)) {
      fail("formal adapter integrity receipt is inside a forbidden root and is not disjoint");
    }
  }
  if (probeBytes?.subarray(0, 2).equals(Buffer.from("#!", "utf8"))) {
    fail(
      "formal adapter probe must be a self-contained hash-bound executable, " +
        "not an interpreted probe with an unsealed interpreter closure",
    );
  }
  if (!SHA256.test(parsed.probe.sha256 ?? "") ||
      (probeBytes && sha256(probeBytes) !== parsed.probe.sha256)) {
    fail("formal adapter probe SHA-256 mismatch");
  }
  const integrityBytes = await readFile(integrityReceipt);
  if (sha256(integrityBytes) !== expectedIntegrityReceiptSha256) {
    fail("formal adapter integrity receipt SHA-256 mismatch");
  }
  const approval = JSON.parse(integrityBytes.toString("utf8"));
  const expectedBindings = {
    protocol_id: context?.protocol_id,
    phase: context?.phase,
    wave_id: context?.wave_id,
    run_id: context?.run_id,
    attempt_id: context?.attempt_id,
    candidate_id: context?.candidate_id,
    case_id: context?.case_id,
    capture_receipt_sha256: context?.capture_receipt_sha256,
    scaffold_sha256: sha256(adapterBytes),
    config_sha256: sha256(configBytes),
    probe_sha256: parsed.probe.sha256,
    candidate_tree: context?.candidate_tree,
  };
  if (approval.schema !== "tachiko-adapter-integrity-review-v1") {
    fail("formal adapter integrity receipt schema mismatch");
  }
  for (const [key, value] of Object.entries(expectedBindings)) {
    if (approval[key] !== value) fail(`formal adapter integrity receipt ${key} mismatch`);
  }
  if (!SHA1.test(context?.candidate_tree ?? "")) {
    fail("formal adapter context lacks a captured candidate tree binding");
  }
  if (
    approval.reviewer_eligible !== true || approval.reviewer_independent !== true ||
    typeof approval.reviewer_id !== "string" || approval.reviewer_id.length === 0 ||
    approval.no_expected_values !== true || approval.no_behavior_implementation !== true ||
    approval.actual_candidate_exercise !== true || approval.self_contained_executable !== true ||
    approval.complete_input_closure !== true || approval.approved !== true
  ) {
    fail("formal adapter integrity receipt lacks an eligible independent approval");
  }
  let probeBuildReceipt = null;
  let adapterBuildStageReceipt = null;
  let probeSource = null;
  let candidateManifest = null;
  if (context.case_id === "TW-05") {
    if (approval.candidate_binding_mode !== "runtime_candidate_root" ||
        approval.probe_build_receipt !== null) {
      fail("TW-05 formal adapter must bind direct runtime candidate exercise");
    }
  } else if (context.case_id === "TW-09") {
    const consumingControllerBuild = builtProbePath !== undefined ||
      probeBuildReceiptPath !== undefined || adapterBuildStageReceiptPath !== undefined;
    if (approval.candidate_binding_mode !== "captured_candidate_build" ||
        approval.probe_build_receipt !== null || !candidateRoot || !probeSourcePath ||
        (!consumingControllerBuild && !buildRoot) || approval.probe_source_sha256 === undefined) {
      fail("TW-09 formal adapter requires a sealed controller candidate-probe build");
    }
    probeSource = await regular(probeSourcePath, "formal TW-09 reviewer probe source");
    for (const root of forbiddenRoots) {
      if (isInside(probeSource, await realpath(root))) {
        fail("formal TW-09 reviewer probe source is inside a forbidden root");
      }
    }
    const sourceBytes = await readFile(probeSource);
    if (sha256(sourceBytes) !== approval.probe_source_sha256) {
      fail("formal adapter integrity receipt probe_source_sha256 mismatch");
    }
    const built = consumingControllerBuild
      ? await verifyControllerBuiltTw09Probe({
        context,
        controllerContextPath,
        candidateRoot,
        candidateManifestPath,
        probeSourcePath: probeSource,
        configIdentity: await identity(config),
        integrityIdentity: await identity(integrityReceipt),
        builtProbePath,
        expectedBuiltProbeSha256,
        probeBuildReceiptPath,
        expectedProbeBuildReceiptSha256,
        adapterBuildStageReceiptPath,
        expectedAdapterBuildStageReceiptSha256,
        cargoPath,
        cargoSha256,
        rustcPath,
        rustcSha256,
      })
      : await buildFormalTw09CandidateProbe({
        candidateRoot,
        candidateTree: context.candidate_tree,
        captureReceiptSha256: context.capture_receipt_sha256,
        rawTreeDigestSha256: context.raw_tree_digest_sha256,
        candidateManifestPath,
        probeSourcePath: probeSource,
        expectedProbeSha256: parsed.probe.sha256,
        buildRoot,
        cargoPath,
        cargoSha256,
        rustcPath,
        rustcSha256,
        environment,
      });
    probe = built.probe.path;
    probeBytes = await readFile(probe);
    probeBuildReceipt = built.probe_build_receipt;
    adapterBuildStageReceipt = built.adapter_build_stage_receipt ?? null;
    candidateManifest = built.candidate_manifest;
  }
  return {
    scaffold: await identity(adapter),
    scaffold_lock: await identity(lockPath),
    config: await identity(config),
    probe: await identity(probe),
    ...(probeSource ? {probe_source: await identity(probeSource)} : {}),
    ...(candidateManifest ? {candidate_manifest: candidateManifest} : {}),
    integrity_receipt: await identity(integrityReceipt),
    ...(probeBuildReceipt ? {probe_build_receipt: probeBuildReceipt} : {}),
    ...(adapterBuildStageReceipt
      ? {adapter_build_stage_receipt: adapterBuildStageReceipt} : {}),
    approval,
  };
}
