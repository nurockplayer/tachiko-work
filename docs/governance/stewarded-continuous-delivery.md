# Stewarded Continuous Delivery

Status: terminology and navigation note; no new delivery authority

Naming decision: [#370](https://github.com/nurockplayer/tachiko-work/issues/370).
The canonical definition of **Stewarded Continuous Delivery (SCD)** is in the
[Repository delivery workflow](project-governance.md#repository-delivery-workflow).

SCD names the existing governed continuous delivery model, not a model/vendor
combination. Project Steward, Mission Lead/orchestrator, implementation worker,
and independent reviewer describe responsibilities; actual agent assignments
remain operational choices under the applicable repository authority.

## Where the rules live

- [Repository delivery workflow](project-governance.md#repository-delivery-workflow):
  readiness, acceptance ownership, handoff, review/merge, reconciliation and stops.
- [Delivery Throughput Policy](delivery-throughput-policy.md): risk-appropriate
  review, validation, material GitHub checkpoints and permitted parallel lanes.
- [PR Decomposition Policy](pr-decomposition-policy.md): coherent delivery slices.
- [Agent delivery continuity](../../AGENTS.md#delivery-continuity): continuation
  and recoverable handoff within those rules.

The [knowledge-authority hierarchy](knowledge-authority.md) remains unchanged.
Using the name creates no Ready or merge permission, mandatory model assignment,
new handoff/watch schema, checkpoint cadence, serial-only rule or scheduler.
Other repositories may reuse the name without importing Tachiko Work's local
permissions; their own accepted delivery policies still govern their work.

## Historical note

The founder identified the Richman4 remake work as the origin of this working
style. That is provenance, not the reusable workflow's name or a source of
cross-repository permissions. Historical references to the actual Richman4
project retain their names.
