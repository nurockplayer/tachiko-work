import type {
  AgentHandoff,
  EvidenceParseContext,
  HumanAction,
  OperationalEvidence,
  ParseFailure,
  ParseFailureReason,
  ParseField,
  ParseResult,
  ReviewSeverity,
  SourceRef,
  StewardVerdict,
  StewardWatch,
  StructuredCommentSource,
} from "./model.js";

const HANDOFF_MARKER = "<!-- agent-handoff:v1 -->";
const STEWARD_MARKER = "<!-- project-steward-watch:v1 -->";
const EVIDENCE_MARKER = "<!-- operational-evidence:v1";
const ENVELOPE_CLOSE = "-->";

const FULL_SHA = /^[0-9a-f]{40}$/;
const POSITIVE_INTEGER = /^[1-9][0-9]*$/;
const CANONICAL_TOKEN = /^[a-z0-9]+(?:[-_:][a-z0-9]+)*$/;
const REPOSITORY_IDENTITY = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/;
const REFERENCE_TOKEN = /^[!-~]{1,128}$/;

type FieldValues = ReadonlyMap<string, string>;

interface ParsedFields {
  readonly ok: true;
  readonly values: FieldValues;
}

function sourceRef(
  source: StructuredCommentSource,
  context: EvidenceParseContext,
): SourceRef {
  return {
    repository: source.metadata.repository,
    sourceId: source.metadata.id,
    sourceKind: source.metadata.kind,
    url: source.metadata.url,
    authorLogin: source.metadata.authorLogin,
    authorAssociation: source.metadata.authorAssociation,
    createdAt: source.metadata.createdAt,
    updatedAt: source.metadata.updatedAt,
    observedIssue: context.issueNumber,
    observedPullRequest: context.pullRequestNumber,
    observedHead: context.headSha,
    observedMain: context.mainSha,
  };
}

function failure(
  reason: ParseFailureReason,
  source: StructuredCommentSource,
  context: EvidenceParseContext,
  field?: ParseField,
): ParseFailure {
  return field === undefined
    ? { ok: false, reason, source: sourceRef(source, context) }
    : { ok: false, reason, field, source: sourceRef(source, context) };
}

function isPositiveInteger(value: number): boolean {
  return Number.isSafeInteger(value) && value > 0;
}

function validateContext(
  source: StructuredCommentSource,
  context: EvidenceParseContext,
): ParseFailure | undefined {
  if (
    context.repository.length > 200 ||
    !REPOSITORY_IDENTITY.test(context.repository) ||
    !isPositiveInteger(context.issueNumber) ||
    !isPositiveInteger(context.pullRequestNumber) ||
    !isCanonicalToken(context.owner) ||
    !FULL_SHA.test(context.headSha) ||
    !FULL_SHA.test(context.mainSha)
  ) {
    return failure("context-invalid", source, context);
  }
  return undefined;
}

function validateSource(
  source: StructuredCommentSource,
  context: EvidenceParseContext,
  requireTopLevel: boolean,
  rejectEdited: boolean,
): ParseFailure | undefined {
  if (rejectEdited && source.metadata.edited) {
    return failure("source-edited", source, context);
  }
  if (!source.metadata.trustedProducer) {
    return failure("producer-untrusted", source, context);
  }
  if (requireTopLevel && !source.metadata.topLevel) {
    return failure("source-not-top-level", source, context);
  }
  if (source.metadata.repository !== context.repository) {
    return failure("identity-mismatch", source, context);
  }
  return undefined;
}

function normalizedLines(body: string): readonly string[] {
  return body.replaceAll("\r\n", "\n").split("\n");
}

function markerFailure(
  lines: readonly string[],
  marker: string,
  source: StructuredCommentSource,
  context: EvidenceParseContext,
): ParseFailure | undefined {
  const count = lines.reduce((total, line) => total + Number(line === marker), 0);
  if (count === 0) {
    return failure("marker-missing", source, context);
  }
  if (count > 1) {
    return failure("marker-duplicate", source, context);
  }
  if (lines[0] !== marker) {
    return failure("marker-not-first-line", source, context);
  }
  return undefined;
}

function parseFields(
  lines: readonly string[],
  allowed: ReadonlySet<string>,
  source: StructuredCommentSource,
  context: EvidenceParseContext,
): ParsedFields | ParseFailure {
  if (lines.length === 0) {
    return failure("header-empty", source, context);
  }

  const values = new Map<string, string>();
  for (const line of lines) {
    const match = /^([A-Z_]+): (.*)$/.exec(line);
    if (match === null) {
      return failure("malformed-line", source, context);
    }
    const key = match[1];
    const value = match[2];
    if (key === undefined || value === undefined) {
      return failure("malformed-line", source, context);
    }
    if (!allowed.has(key)) {
      return failure("unknown-field", source, context, asParseField(key));
    }
    if (values.has(key)) {
      return failure("duplicate-field", source, context, asParseField(key));
    }
    values.set(key, value);
  }
  return { ok: true, values };
}

function asParseField(field: string): ParseField | undefined {
  switch (field) {
    case "ISSUE":
    case "PR":
    case "OWNER":
    case "STATE":
    case "HEAD":
    case "MAIN":
    case "VERDICT":
    case "HUMAN_ACTION":
    case "KIND":
    case "RUN":
    case "NAME":
    case "RESULT":
    case "SUPERSEDES":
    case "SEVERITY":
    case "RESOLVES":
      return field;
    default:
      return undefined;
  }
}

function requireFields(
  values: FieldValues,
  required: readonly ParseField[],
  source: StructuredCommentSource,
  context: EvidenceParseContext,
): ParseFailure | undefined {
  for (const field of required) {
    if (!values.has(field)) {
      return failure("missing-field", source, context, field);
    }
  }
  return undefined;
}

function parsePositiveInteger(
  value: string,
  field: ParseField,
  source: StructuredCommentSource,
  context: EvidenceParseContext,
): number | ParseFailure {
  if (!POSITIVE_INTEGER.test(value)) {
    return failure("invalid-positive-integer", source, context, field);
  }
  const parsed = Number(value);
  if (!isPositiveInteger(parsed)) {
    return failure("invalid-positive-integer", source, context, field);
  }
  return parsed;
}

function isCanonicalToken(value: string): boolean {
  return value.length >= 1 && value.length <= 64 && CANONICAL_TOKEN.test(value);
}

function tokenFailure(
  value: string,
  field: ParseField,
  source: StructuredCommentSource,
  context: EvidenceParseContext,
): ParseFailure | undefined {
  return isCanonicalToken(value)
    ? undefined
    : failure("invalid-canonical-token", source, context, field);
}

function referenceFailure(
  value: string,
  field: ParseField,
  source: StructuredCommentSource,
  context: EvidenceParseContext,
): ParseFailure | undefined {
  return REFERENCE_TOKEN.test(value)
    ? undefined
    : failure("invalid-reference-token", source, context, field);
}

function shaFailure(
  value: string,
  field: "HEAD" | "MAIN",
  source: StructuredCommentSource,
  context: EvidenceParseContext,
): ParseFailure | undefined {
  return FULL_SHA.test(value)
    ? undefined
    : failure("invalid-sha", source, context, field);
}

function contiguousHeader(lines: readonly string[]): readonly string[] {
  const header: string[] = [];
  for (let index = 1; index < lines.length; index += 1) {
    const line = lines[index];
    if (line === undefined || line === "") {
      break;
    }
    header.push(line);
  }
  return header;
}

export function parseAgentHandoff(
  source: StructuredCommentSource,
  context: EvidenceParseContext,
): ParseResult<AgentHandoff> {
  const contextProblem = validateContext(source, context);
  if (contextProblem !== undefined) return contextProblem;

  const lines = normalizedLines(source.body);
  const markerProblem = markerFailure(lines, HANDOFF_MARKER, source, context);
  if (markerProblem !== undefined) return markerProblem;
  const sourceProblem = validateSource(source, context, true, false);
  if (sourceProblem !== undefined) return sourceProblem;

  const parsed = parseFields(
    contiguousHeader(lines),
    new Set(["ISSUE", "PR", "OWNER", "STATE", "HEAD", "MAIN"]),
    source,
    context,
  );
  if (!parsed.ok) return parsed;

  const requiredProblem = requireFields(
    parsed.values,
    ["ISSUE", "PR", "OWNER", "STATE", "HEAD", "MAIN"],
    source,
    context,
  );
  if (requiredProblem !== undefined) return requiredProblem;

  const issueValue = parsed.values.get("ISSUE");
  const prValue = parsed.values.get("PR");
  const owner = parsed.values.get("OWNER");
  const state = parsed.values.get("STATE");
  const head = parsed.values.get("HEAD");
  const main = parsed.values.get("MAIN");
  if (
    issueValue === undefined ||
    prValue === undefined ||
    owner === undefined ||
    state === undefined ||
    head === undefined ||
    main === undefined
  ) {
    return failure("missing-field", source, context);
  }

  const issue = parsePositiveInteger(issueValue, "ISSUE", source, context);
  if (typeof issue !== "number") return issue;
  const pullRequest = parsePositiveInteger(prValue, "PR", source, context);
  if (typeof pullRequest !== "number") return pullRequest;

  const ownerProblem = tokenFailure(owner, "OWNER", source, context);
  if (ownerProblem !== undefined) return ownerProblem;
  const stateProblem = tokenFailure(state, "STATE", source, context);
  if (stateProblem !== undefined) return stateProblem;
  const headProblem = shaFailure(head, "HEAD", source, context);
  if (headProblem !== undefined) return headProblem;
  const mainProblem = shaFailure(main, "MAIN", source, context);
  if (mainProblem !== undefined) return mainProblem;

  if (
    issue !== context.issueNumber ||
    pullRequest !== context.pullRequestNumber ||
    owner !== context.owner
  ) {
    return failure("identity-mismatch", source, context);
  }
  if (head !== context.headSha) {
    return failure("head-mismatch", source, context, "HEAD");
  }
  if (main !== context.mainSha) {
    return failure("main-mismatch", source, context, "MAIN");
  }

  const ref = sourceRef(source, context);
  const value: AgentHandoff = {
    kind: "agent-handoff",
    issue,
    pullRequest,
    owner,
    state,
    head,
    main,
    source: ref,
  };
  return { ok: true, value, source: ref };
}

export function parseStewardWatch(
  source: StructuredCommentSource,
  context: EvidenceParseContext,
): ParseResult<StewardWatch> {
  const contextProblem = validateContext(source, context);
  if (contextProblem !== undefined) return contextProblem;

  const lines = normalizedLines(source.body);
  const markerProblem = markerFailure(lines, STEWARD_MARKER, source, context);
  if (markerProblem !== undefined) return markerProblem;
  const sourceProblem = validateSource(source, context, true, false);
  if (sourceProblem !== undefined) return sourceProblem;

  const parsed = parseFields(
    contiguousHeader(lines),
    new Set(["VERDICT", "HEAD", "MAIN", "HUMAN_ACTION"]),
    source,
    context,
  );
  if (!parsed.ok) return parsed;
  const requiredProblem = requireFields(
    parsed.values,
    ["VERDICT", "HEAD", "MAIN", "HUMAN_ACTION"],
    source,
    context,
  );
  if (requiredProblem !== undefined) return requiredProblem;

  const verdict = parsed.values.get("VERDICT");
  const head = parsed.values.get("HEAD");
  const main = parsed.values.get("MAIN");
  const humanAction = parsed.values.get("HUMAN_ACTION");
  if (
    verdict === undefined ||
    head === undefined ||
    main === undefined ||
    humanAction === undefined
  ) {
    return failure("missing-field", source, context);
  }
  if (verdict !== "GREEN" && verdict !== "AMBER" && verdict !== "HOLD") {
    return failure("invalid-enum", source, context, "VERDICT");
  }
  if (humanAction !== "none" && humanAction !== "required") {
    return failure("invalid-enum", source, context, "HUMAN_ACTION");
  }
  const headProblem = shaFailure(head, "HEAD", source, context);
  if (headProblem !== undefined) return headProblem;
  const mainProblem = shaFailure(main, "MAIN", source, context);
  if (mainProblem !== undefined) return mainProblem;
  if (head !== context.headSha) {
    return failure("head-mismatch", source, context, "HEAD");
  }
  if (main !== context.mainSha) {
    return failure("main-mismatch", source, context, "MAIN");
  }

  const ref = sourceRef(source, context);
  const value: StewardWatch = {
    kind: "project-steward-watch",
    verdict: verdict as StewardVerdict,
    head,
    main,
    humanAction: humanAction as HumanAction,
    source: ref,
  };
  return { ok: true, value, source: ref };
}

const ALL_OPERATIONAL_FIELDS = new Set([
  "KIND",
  "PR",
  "HEAD",
  "RUN",
  "NAME",
  "RESULT",
  "SUPERSEDES",
  "SEVERITY",
  "RESOLVES",
]);

function envelopeLines(
  lines: readonly string[],
  source: StructuredCommentSource,
  context: EvidenceParseContext,
): readonly string[] | ParseFailure {
  const markerCount = lines.reduce(
    (total, line) => total + Number(line === EVIDENCE_MARKER),
    0,
  );
  if (markerCount === 0) {
    return failure("marker-missing", source, context);
  }
  if (markerCount > 1) {
    return failure("multiple-envelopes", source, context);
  }
  if (lines[0] !== EVIDENCE_MARKER) {
    return failure("marker-not-first-line", source, context);
  }
  const closeIndex = lines.indexOf(ENVELOPE_CLOSE, 1);
  if (closeIndex === -1) {
    return failure("envelope-unclosed", source, context);
  }
  return lines.slice(1, closeIndex);
}

export function parseOperationalEvidence(
  source: StructuredCommentSource,
  context: EvidenceParseContext,
): ParseResult<OperationalEvidence> {
  const contextProblem = validateContext(source, context);
  if (contextProblem !== undefined) return contextProblem;

  const lines = normalizedLines(source.body);
  const envelope = envelopeLines(lines, source, context);
  if ("ok" in envelope) return envelope;
  const sourceProblem = validateSource(source, context, false, true);
  if (sourceProblem !== undefined) return sourceProblem;

  const parsed = parseFields(envelope, ALL_OPERATIONAL_FIELDS, source, context);
  if (!parsed.ok) return parsed;
  const kind = parsed.values.get("KIND");
  if (
    kind !== "validation" &&
    kind !== "review" &&
    kind !== "review-finding" &&
    kind !== "review-resolution"
  ) {
    return failure("invalid-enum", source, context, "KIND");
  }

  const allowedForKind: ReadonlySet<string> =
    kind === "validation" || kind === "review"
      ? new Set(["KIND", "PR", "HEAD", "RUN", "NAME", "RESULT", "SUPERSEDES"])
      : kind === "review-finding"
        ? new Set(["KIND", "PR", "HEAD", "RUN", "SEVERITY"])
        : new Set(["KIND", "PR", "HEAD", "RESOLVES"]);
  for (const key of parsed.values.keys()) {
    if (!allowedForKind.has(key)) {
      return failure("unknown-field", source, context, asParseField(key));
    }
  }

  const required: readonly ParseField[] =
    kind === "validation" || kind === "review"
      ? ["KIND", "PR", "HEAD", "RUN", "NAME", "RESULT"]
      : kind === "review-finding"
        ? ["KIND", "PR", "HEAD", "RUN", "SEVERITY"]
        : ["KIND", "PR", "HEAD", "RESOLVES"];
  const requiredProblem = requireFields(parsed.values, required, source, context);
  if (requiredProblem !== undefined) return requiredProblem;

  const prValue = parsed.values.get("PR");
  const head = parsed.values.get("HEAD");
  if (prValue === undefined || head === undefined) {
    return failure("missing-field", source, context);
  }
  const pullRequest = parsePositiveInteger(prValue, "PR", source, context);
  if (typeof pullRequest !== "number") return pullRequest;
  const headProblem = shaFailure(head, "HEAD", source, context);
  if (headProblem !== undefined) return headProblem;
  if (pullRequest !== context.pullRequestNumber) {
    return failure("identity-mismatch", source, context, "PR");
  }
  const ref = sourceRef(source, context);
  if (kind === "validation" || kind === "review") {
    const run = parsed.values.get("RUN");
    const name = parsed.values.get("NAME");
    const result = parsed.values.get("RESULT");
    if (run === undefined || name === undefined || result === undefined) {
      return failure("missing-field", source, context);
    }
    const runProblem = referenceFailure(run, "RUN", source, context);
    if (runProblem !== undefined) return runProblem;
    const nameProblem = tokenFailure(name, "NAME", source, context);
    if (nameProblem !== undefined) return nameProblem;
    const supersedes = parsed.values.get("SUPERSEDES");
    if (supersedes !== undefined) {
      const supersedesProblem = referenceFailure(
        supersedes,
        "SUPERSEDES",
        source,
        context,
      );
      if (supersedesProblem !== undefined) return supersedesProblem;
    }

    if (kind === "validation") {
      if (result !== "pass" && result !== "fail" && result !== "unknown") {
        return failure("incompatible-result", source, context, "RESULT");
      }
      const value: OperationalEvidence = {
        kind,
        pullRequest,
        head,
        run,
        name,
        result,
        ...(supersedes === undefined ? {} : { supersedes }),
        source: ref,
      };
      return { ok: true, value, source: ref };
    }

    if (result !== "clean" && result !== "findings" && result !== "unknown") {
      return failure("incompatible-result", source, context, "RESULT");
    }
    const value: OperationalEvidence = {
      kind,
      pullRequest,
      head,
      run,
      name,
      result,
      ...(supersedes === undefined ? {} : { supersedes }),
      source: ref,
    };
    return { ok: true, value, source: ref };
  }

  if (kind === "review-finding") {
    const run = parsed.values.get("RUN");
    const severity = parsed.values.get("SEVERITY");
    if (run === undefined || severity === undefined) {
      return failure("missing-field", source, context);
    }
    const runProblem = referenceFailure(run, "RUN", source, context);
    if (runProblem !== undefined) return runProblem;
    if (
      severity !== "P0" &&
      severity !== "P1" &&
      severity !== "P2" &&
      severity !== "P3"
    ) {
      return failure("invalid-enum", source, context, "SEVERITY");
    }
    const value: OperationalEvidence = {
      kind,
      pullRequest,
      head,
      run,
      severity: severity as ReviewSeverity,
      source: ref,
    };
    return { ok: true, value, source: ref };
  }

  const resolves = parsed.values.get("RESOLVES");
  if (resolves === undefined) {
    return failure("missing-field", source, context, "RESOLVES");
  }
  const resolvesProblem = referenceFailure(resolves, "RESOLVES", source, context);
  if (resolvesProblem !== undefined) return resolvesProblem;
  const value: OperationalEvidence = {
    kind,
    pullRequest,
    head,
    resolves,
    source: ref,
  };
  return { ok: true, value, source: ref };
}
