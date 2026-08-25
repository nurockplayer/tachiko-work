import {createHash} from "node:crypto";

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

export function contentSha256(value) {
  return sha256(`${JSON.stringify(value)}\n`);
}

function deterministicSupervision(supervision) {
  if (!supervision) return null;
  return {
    deadline_seconds: supervision.deadline_seconds,
    exit_code: supervision.exit_code,
    signal: supervision.signal,
    spawn_error: supervision.spawn_error,
    timed_out: supervision.timed_out,
    process_group_created: supervision.process_group_created,
    termination_grace_seconds: supervision.termination_grace_seconds,
    termination_grace_intervals: supervision.termination_grace_intervals,
    termination_deadline_reused_for_cleanup: supervision.termination_deadline_reused_for_cleanup,
    termination_signal_sent: supervision.termination_signal_sent,
    kill_signal_sent: supervision.kill_signal_sent,
    signals_sent: supervision.signal_actions.map((entry) => entry.signal),
    descendant_cleanup_required: supervision.descendant_cleanup_required,
    process_group_extinct_before_capture: supervision.process_group_extinct_before_capture,
  };
}

function deterministicCommandSupervision(supervision) {
  if (!supervision) return null;
  const deadlines = supervision.stage_processes.map((entry) => entry.deadline_milliseconds);
  return {
    deadline_seconds: supervision.deadline_seconds,
    stage_processes: supervision.stage_processes.map((entry) => ({
      name: entry.name,
      deadline_positive: entry.deadline_milliseconds > 0,
      deadline_within_command:
        entry.deadline_milliseconds <= supervision.deadline_seconds * 1000,
      process_supervision: (() => {
        const stable = deterministicSupervision(entry.process_supervision);
        delete stable.deadline_seconds;
        return stable;
      })(),
    })),
    stage_deadlines_nonincreasing: deadlines.every(
      (deadline, index) => index === 0 || deadline <= deadlines[index - 1],
    ),
    all_process_groups_extinct_before_capture:
      supervision.all_process_groups_extinct_before_capture,
  };
}

function deterministicCommand(command) {
  const commandProcessSupervision = deterministicSupervision(command.process_supervision);
  if (command.command_supervision) delete commandProcessSupervision.deadline_seconds;
  return {
    id: command.id,
    command_template_sha256: command.command_template_sha256,
    resolved_command_bound_in_run_receipt: Boolean(command.resolved_command_sha256),
    execution_mode: command.execution_mode ?? "shell",
    exit_code: command.exit_code,
    signal: command.signal,
    spawn_error: command.spawn_error,
    process_supervision: commandProcessSupervision,
    command_supervision: deterministicCommandSupervision(command.command_supervision),
    ...(command.rust_build ? {
      toolchain: {
        cargo_sha256: command.toolchain.cargo.sha256,
        cargo_bytes: command.toolchain.cargo.bytes,
        rustc_sha256: command.toolchain.rustc.sha256,
        rustc_bytes: command.toolchain.rustc.bytes,
      },
      rust_build: {
        metadata_process_supervision: (() => {
          const stable = deterministicSupervision(command.rust_build.metadata_process_supervision);
          if (stable) delete stable.deadline_seconds;
          return stable;
        })(),
        build_process_supervision: (() => {
          const stable = deterministicSupervision(command.rust_build.build_process_supervision);
          if (stable) delete stable.deadline_seconds;
          return stable;
        })(),
        package: command.rust_build.package ? {
          name: command.rust_build.package.name,
          manifest_sha256: command.rust_build.package.manifest_sha256,
          target_name: command.rust_build.package.target_name,
          target_source_sha256: command.rust_build.package.target_source_sha256,
        } : null,
        artifact_message_present: Boolean(command.rust_build.artifact?.message_sha256),
        executable_present: Boolean(command.rust_build.artifact?.executable_sha256),
      },
    } : {}),
  };
}

function reasonClass(reason) {
  const classes = [
    "command exited",
    "matching Rust tests",
    "matching Rust test lacks",
    "libtest JSON",
    "JSON pointer",
    "native selected",
    "WASM selected",
    "native/WASM selected",
    "portable observations unavailable",
    "selected native records",
    "selected WASM records",
  ];
  return classes.find((prefix) => reason.startsWith(prefix)) ?? "other_selector_failure";
}

function deterministicSuiteSummary(summary) {
  if (summary === null || summary === undefined) return summary;
  return Object.fromEntries(Object.entries(summary).filter(
    ([key]) => !["exec_time", "duration", "duration_ms"].includes(key),
  ));
}

function deterministicAssertion(assertion) {
  return Object.fromEntries(Object.entries({
    id: assertion.id,
    command_id: assertion.command_id,
    selector_kind: assertion.selector_kind,
    pass: assertion.pass,
    reason_classes: [...new Set(assertion.reasons.map(reasonClass))],
    evidence_mode: assertion.evidence_mode,
    matching_tests: assertion.matching_tests,
    matching_test_outcomes: assertion.matching_test_outcomes,
    required_matching_tests: assertion.required_matching_tests,
    suite_summary: deterministicSuiteSummary(assertion.suite_summary),
    normalized_events_sha256: assertion.normalized_events_sha256,
    normalized_suite_sha256: assertion.normalized_suite_sha256,
    json_pointer: assertion.json_pointer,
    found: assertion.found,
    actual_canonical_sha256: assertion.actual_canonical_sha256,
    selected_native_sha256: assertion.selected_native_sha256,
    selected_wasm_sha256: assertion.selected_wasm_sha256,
  }).filter(([, value]) => value !== undefined));
}

function deterministicAdapter(adapter) {
  if (!adapter) return null;
  const stableObservation = adapter.observation === null
    ? null
    : Object.fromEntries(Object.entries(adapter.observation).filter(
      ([key]) => ![
        "cargo_stdout_sha256",
        "cargo_stderr_sha256",
        "build_chatter_sha256",
        "run_root",
      ].includes(key),
    ));
  return {
    kind: adapter.kind,
    command_exit_code: adapter.command_exit_code,
    observation: stableObservation,
    trusted_inputs: adapter.trusted_inputs,
    observation_artifact_present: adapter.observation_artifact !== null,
  };
}

function deterministicOracle(oracle) {
  return {
    evidence: oracle.evidence,
    process_exit_code: oracle.process_exit_code,
    runner_process_supervision: deterministicSupervision(oracle.runner_process_supervision),
    assessment_mode: oracle.assessment_mode,
    overall_status: oracle.overall_status,
    commands_pass: oracle.commands_pass,
    assertions_pass: oracle.assertions_pass,
    commands: oracle.commands.map(deterministicCommand),
    assertions: oracle.assertions.map(deterministicAssertion),
    adapter_execution: deterministicAdapter(oracle.adapter_execution),
    network_enforcement: oracle.network_enforcement ? {
      mode: oracle.network_enforcement.mode,
      sandbox_executable: oracle.network_enforcement.sandbox_executable ? {
        bytes: oracle.network_enforcement.sandbox_executable.bytes,
        sha256: oracle.network_enforcement.sandbox_executable.sha256,
      } : null,
      profile: oracle.network_enforcement.profile,
      probe_script: oracle.network_enforcement.probe_script ? {
        bytes: oracle.network_enforcement.probe_script.bytes,
        sha256: oracle.network_enforcement.probe_script.sha256,
      } : null,
      probe_denied: oracle.network_enforcement.probe_denied,
      process_supervision: deterministicSupervision(
        oracle.network_enforcement.process_supervision,
      ),
    } : null,
  };
}

function deterministicCore(core) {
  return {
    evidence: core.evidence,
    all_passed: core.all_passed,
    commands: core.commands.map(deterministicCommand),
  };
}

function deterministicOffline(offline) {
  if (!offline) return undefined;
  return {
    evidence: offline.evidence,
    process_exit_code: offline.process_exit_code,
    pass: offline.pass,
    offline: offline.offline,
    package_manager_dependency: offline.package_manager_dependency,
    process_supervision: deterministicSupervision(offline.process_supervision),
    network_enforcement: offline.network_enforcement,
    executables: offline.executables,
    executions: offline.executions.map(({purpose, name, args, exit_code, signal, spawn_error}) => ({
      purpose, name, args, exit_code, signal, spawn_error,
    })),
  };
}

function deterministicCase(entry) {
  return {
    case_id: entry.case_id,
    materialization: entry.materialization,
    qualification: entry.qualification,
    machine_semantic_discrimination_qualified: entry.machine_semantic_discrimination_qualified,
    target: {
      accepted: entry.target.accepted,
      expected_contract_miss: entry.target.expected_contract_miss,
      calibration: entry.target.calibration,
      core: deterministicCore(entry.target.core),
      oracle: deterministicOracle(entry.target.oracle),
    },
    negative: {
      discriminated: entry.negative.discriminated,
      core: deterministicCore(entry.negative.core),
      oracle: deterministicOracle(entry.negative.oracle),
    },
    ...(entry.offline_historical_target ? {
      offline_historical_target: deterministicOffline(entry.offline_historical_target),
      offline_behavior_missing_negative: deterministicOffline(entry.offline_behavior_missing_negative),
    } : {}),
    ...(entry.reference_positive ? {
      reference_positive: {
        accepted: entry.reference_positive.accepted,
        label: entry.reference_positive.label,
        oracle: deterministicOracle(entry.reference_positive.oracle),
      },
    } : {}),
  };
}

export function deterministicPayload(runReceipt) {
  const normalizedEvidence = {
    families: runReceipt.families,
    cases: runReceipt.cases.map(deterministicCase),
  };
  const evidenceCommitmentSha256 = contentSha256(normalizedEvidence);
  return {
    schema: "tachiko-oracle-qualification-summary-v3",
    protocol_id: runReceipt.protocol_id,
    classification: runReceipt.classification,
    formal_result_eligible: runReceipt.formal_result_eligible,
    execution_standard: runReceipt.execution_standard,
    mode: runReceipt.mode,
    no_codex_launched: runReceipt.no_codex_launched,
    trusted_cargo: {
      bytes: runReceipt.trusted_cargo.bytes,
      sha256: runReceipt.trusted_cargo.sha256,
    },
    trusted_rustc: {
      bytes: runReceipt.trusted_rustc.bytes,
      sha256: runReceipt.trusted_rustc.sha256,
    },
    trusted_shell: {
      bytes: runReceipt.trusted_shell.bytes,
      sha256: runReceipt.trusted_shell.sha256,
      version: runReceipt.trusted_shell.version,
    },
    expected_control_sha256: runReceipt.expected_control_sha256,
    controls: runReceipt.controls,
    frozen_manifest_sha256: runReceipt.frozen_manifest_sha256,
    frozen_oracle_lock_sha256: runReceipt.frozen_oracle_lock_sha256,
    network_enforcement: {
      ...runReceipt.network_enforcement,
      process_supervision: deterministicSupervision(
        runReceipt.network_enforcement.process_supervision,
      ),
    },
    evidence_commitment_sha256: evidenceCommitmentSha256,
    ...normalizedEvidence,
    limitations: runReceipt.limitations,
  };
}

export function verifyQualificationReceipt(receipt) {
  if (contentSha256(receipt.run_receipt) !== receipt.run_receipt_sha256) {
    throw new Error("qualification run receipt SHA-256 mismatch");
  }
  const expectedPayload = deterministicPayload(receipt.run_receipt);
  if (JSON.stringify(expectedPayload) !== JSON.stringify(receipt.payload)) {
    throw new Error("deterministic summary does not match run evidence");
  }
  if (contentSha256(receipt.payload) !== receipt.payload_sha256) {
    throw new Error("qualification deterministic payload SHA-256 mismatch");
  }
  if (
    receipt.evidence_commitment_sha256 !== expectedPayload.evidence_commitment_sha256 ||
    receipt.payload.evidence_commitment_sha256 !== expectedPayload.evidence_commitment_sha256
  ) {
    throw new Error("qualification evidence commitment mismatch");
  }
  return {
    payload_sha256: receipt.payload_sha256,
    run_receipt_sha256: receipt.run_receipt_sha256,
    evidence_commitment_sha256: receipt.evidence_commitment_sha256,
  };
}
