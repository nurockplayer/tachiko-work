# Security Policy

## Supported versions

There is no published external release yet. Before the first release, security
fixes target the current `0.1.x` development line. After publication, the latest
tagged `0.1.x` release is the supported release; users of an older `0.1.x`
version should upgrade to the current patch unless its release notes explicitly
say otherwise. Unofficial builds and versions older than `0.1.x` are not
supported distributions.

## Current threat scope

Tachiko Work is currently a local Rust CLI. It parses untrusted `.ro` documents,
validates typed semantic data, evaluates formulas, performs semantic diff and
merge, and writes only explicitly requested new outputs. Formula text is parsed
as data and must never become arbitrary host-code execution. AI-facing APIs are
read/explain/suggest-only; suggestions require explicit approval before a
separate mutation workflow.

Relevant reports include unsafe archive or path handling, arbitrary code
execution, resource-exhaustion attacks from crafted documents, validation or
no-overwrite bypasses, nondeterminism that compromises review integrity, and AI
operations that mutate data or bypass approval. The current product has no
cloud service, realtime collaboration service, plugin runtime, Office importer,
or integrated remote model, so reports should distinguish implemented behavior
from future design documents.

Release archives have SHA-256 checksums but are not signed, and macOS binaries
are not notarized. A checksum detects corruption relative to the published
checksum; it is not a substitute for signing or release-account security.

## Report a vulnerability privately

Do not disclose exploit details, reproduction steps, sensitive paths, or user
data in a public issue or pull request.

This repository does not currently advertise an enabled private vulnerability
reporting channel or a security contact. If the issue tracker is available to
you, open a minimal repository issue titled
`Private security reporting channel requested` and include only the request for
a maintainer to establish a private channel. Do not include the vulnerability's
type, affected component, impact, version, or any evidence in that issue. If you
cannot open an issue, do not move the details to another public channel; wait
until a maintainer advertises a private path. Share details only through that
private path.

If GitHub private vulnerability reporting becomes enabled for this repository,
use **Security → Report a vulnerability** instead, and update this policy to
make that the primary path.

Once a private channel exists, include the affected version or commit, impact,
minimal reproduction, relevant configuration, and any known mitigation. The
project does not promise a response or remediation timeline at this stage;
coordinate public disclosure with the maintainer after private acknowledgement.
