# Issue #601: test and harness peak memory

Measurement probe, research only. It changes no default Node version, CI image,
lockfile or product code.

- `measure-tree.mjs`: runs one command and samples its whole process tree from
  `/proc` every 200 ms (Linux only). It reports peak tree PSS, summed RSS, the
  split by category (rustc, linker, cargo, node, browser, test binaries), and
  the largest single process.
- `heap-probe.mjs`: loaded with `NODE_OPTIONS=--import` into every Node
  process, it records peak `process.memoryUsage()` values.
- `run.sh rust <jobs> | tests | node`: the driver. Run it on a hosted runner,
  never on the developer Mac:

```sh
remote-test -- 'bash docs/research/probes/issue-601-test-memory/run.sh rust 4'
remote-test -- 'bash docs/research/probes/issue-601-test-memory/run.sh tests'
remote-test -- 'REPS=5 bash docs/research/probes/issue-601-test-memory/run.sh node'
```

Results and recommendation: Issue #601 (2026-11-02 JST).
