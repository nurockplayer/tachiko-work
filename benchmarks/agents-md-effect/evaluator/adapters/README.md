# Trusted candidate-specific adapter protocol

TW-05 and TW-09 intentionally permit blinded name/type translation because the
tasks do not freeze candidate-internal APIs. The permanent historical adapters
exercise the frozen historical target APIs directly. A future candidate whose
names differ uses `candidate-adapter.mjs` plus one reviewer-authored config and
probe.

The controller pauses the same attempt after trusted capture, without launching
or sampling another agent. A trusted oracle custodian writes a probe that
invokes candidate production behavior and prints only normalized actual
observations. The config binds that non-symlink probe by absolute path and
SHA-256. Both files must remain outside the candidate workspace and disjoint
from expected/control roots. Formal use requires the hash-locked evaluator
scaffold plus an eligible independent integrity approval bound to the phase,
attempt, capture, scaffold, config, and probe.

The controller passes the config through `run-oracles.mjs --adapter-config`.
One outer Darwin sandbox denies network, expected/control reads, and writes to
the candidate and trusted inputs; only a new empty adapter TMP is writable. The
scaffold receives no expected values or trusted output path, emits one envelope
receipt on stdout, and exits. After process-group extinction and unchanged
pre/post identities, the controller exclusively materializes the normalized
output. Missing, candidate-owned, symlinked, hash-mismatched, nonzero, malformed,
unreviewed, behavior-implementing, or unresolved inputs fail the same attempt.
Creating the adapter is a review stage, not resampling, and cannot change the
frozen task, selector, points, or candidate tree.
