# Operational evidence

This private repository-local library implements the strict structured-evidence
boundary defined by GitHub Issue #200 and the repository delivery workflow. It
has no GitHub client, UI, write capability, scheduler, or Tachiko product
semantics.

Consumers supply complete GitHub context explicitly: enclosing Issue and pull
request identities, observed PR head and live `main`, trusted producer metadata,
native check/review/thread observations, candidate structured comments, and the
requirements being evaluated. The library returns discriminated parse results
and finite, source-linked reconciliation conditions. It never infers authority
from narrative prose.

Gate reconciliation also requires a completeness-aware native repository
observation covering structured Issue readiness, dependencies, authority
conflict, and PR state/draft/base facts. Validation and review requirements
declare whether their authoritative producer is native GitHub evidence or an
explicit manual/local gap; a custom envelope cannot substitute for a missing
native-required check or review.

The public entry point is `src/index.ts`. A later repository-local Dashboard can
depend on this package directly and provide its own read-only GitHub adapter.

Run the focused gate from the repository root:

```sh
bash scripts/operational-evidence-check.sh
```

The gate uses pnpm 11.25.0, typechecks the strict TypeScript surface, runs the
deterministic fixture suite, and emits JavaScript plus declarations in the
ignored `dist/` directory.
