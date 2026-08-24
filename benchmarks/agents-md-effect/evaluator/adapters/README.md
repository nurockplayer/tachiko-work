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
SHA-256. Both files must remain outside the candidate workspace. The scaffold
executes the probe under Darwin kernel network denial, wraps its actual output
with contract and adapter identities, and never supplies expected values to the
probe.

The controller passes the config through `run-oracles.mjs --adapter-config`.
The runner records the config bytes, adapter bytes, contract bytes, and control
digest. Missing, candidate-owned, symlinked, hash-mismatched, nonzero, malformed,
or unresolved inputs fail the same attempt. Creating the adapter is a review
stage, not resampling, and cannot change the frozen task, selector, points, or
candidate tree.
