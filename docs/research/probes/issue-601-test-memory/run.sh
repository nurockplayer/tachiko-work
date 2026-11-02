#!/usr/bin/env bash
# Issue #601 measurement driver, meant for the Remote check workflow
# (ubuntu-24.04 hosted runner) through `remote-test`, never the developer Mac:
#   remote-test -- 'bash docs/research/probes/issue-601-test-memory/run.sh rust 4'
#   remote-test -- 'bash docs/research/probes/issue-601-test-memory/run.sh node'
# Every measured command prints one `MEASURE {json}` line; the summary at the
# end repeats them so they survive the log tail.
set -euo pipefail

probe_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${probe_dir}/../../../.." && pwd)"
measure="${probe_dir}/measure-tree.mjs"
results="${RUNNER_TEMP:-/tmp}/issue-601-results.jsonl"
reps="${REPS:-3}"
: >"${results}"
cd "${repo_root}"

# record <label> <command...>: sample the command's process tree from here.
record() {
  local label="$1"
  shift
  capture node "${measure}" --label "${label}" -- "$@"
}

# capture <command...>: the command prints its own MEASURE line (containers
# run the sampler inside, so it sees the container's processes).
capture() {
  local raw="${RUNNER_TEMP:-/tmp}/issue-601-last.log"
  "$@" >"${raw}" 2>&1 || true
  grep '^MEASURE ' "${raw}" | sed 's/^MEASURE //' >>"${results}" || true
  tail -n 1 "${results}"
  if ! tail -n 1 "${results}" | grep -q '"exit":0,'; then
    echo "-- last output lines of the failed command:"
    grep -v '^MEASURE ' "${raw}" | tail -n 40
  fi
}

environment() {
  echo "== environment"
  uname -srm
  nproc
  free -m
  node --version
  cargo --version
  rustc --version
  docker --version 2>/dev/null || true
}

touch_workspace_sources() {
  cargo metadata --no-deps --format-version 1 --locked |
    node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{for(const p of JSON.parse(s).packages)console.log(require("path").dirname(p.manifest_path))})' |
    while read -r dir; do find "${dir}/src" -name '*.rs' -exec touch {} +; done
}

rust_mode() {
  local jobs="$1"
  local cold_target="${RUNNER_TEMP:-/tmp}/issue-601-cold-target"
  environment
  echo "== cold: full workspace test build from an empty target dir, -j ${jobs}"
  rm -rf "${cold_target}"
  record "rust cold build -j${jobs}" env CARGO_TARGET_DIR="${cold_target}" \
    cargo test --workspace --all-targets --locked --no-run -j "${jobs}"
  rm -rf "${cold_target}"

  echo "== warm deps: workspace crates rebuilt (sources touched), -j ${jobs}"
  cargo test --workspace --all-targets --locked --no-run -j "${jobs}" >/dev/null 2>&1
  for rep in $(seq "${reps}"); do
    touch_workspace_sources
    record "rust workspace rebuild -j${jobs} #${rep}" \
      cargo test --workspace --all-targets --locked --no-run -j "${jobs}"
  done

  if [[ "${jobs}" == "4" && -z "${SKIP_EXTRA:-}" ]]; then
    pnpm --dir experiments/open-sheet-export install --frozen-lockfile >/dev/null
    echo "== test execution only (binaries already built)"
    for rep in $(seq "${reps}"); do
      record "rust workspace test run #${rep}" cargo test --workspace --all-targets --locked
    done
    echo "== worker-typical: runtime crate interop_adapter (build + run, warm deps)"
    local runtime=packages/browser-client/runtime/Cargo.toml
    cargo test --manifest-path "${runtime}" --locked --test interop_adapter --no-run >/dev/null 2>&1
    for rep in $(seq "${reps}"); do
      find packages/browser-client/runtime/src -name '*.rs' -exec touch {} +
      record "runtime interop_adapter rebuild+run #${rep}" \
        cargo test --manifest-path "${runtime}" --locked --test interop_adapter
    done
  fi
}

node_suite_commands=(
  "browser-client vitest|packages/browser-client|node node_modules/vitest/vitest.mjs run"
  "project-dashboard vitest|tools/project-dashboard|node node_modules/vitest/vitest.mjs run"
)

node_mode() {
  environment
  pnpm --version
  pnpm --dir packages/browser-client install --frozen-lockfile
  pnpm --dir tools/project-dashboard install --frozen-lockfile
  local images=(node:24-slim node:26-slim platformatic/node-caged:26-slim)
  for image in "${images[@]}"; do
    docker pull --quiet "${image}"
    echo "image ${image} $(docker inspect --format '{{index .RepoDigests 0}}' "${image}")"
    docker run --rm "${image}" node -p 'process.version + " " + process.arch + " pointerCompression=" + (process.config.variables.v8_enable_pointer_compression ?? "?")'
  done
  for suite in "${node_suite_commands[@]}"; do
    IFS='|' read -r name dir cmd <<<"${suite}"
    for image in "${images[@]}"; do
      for rep in $(seq "${reps}"); do
        local heap_dir="${repo_root}/.issue-601-heap"
        rm -rf "${heap_dir}" && mkdir -p "${heap_dir}"
        capture docker run --rm \
          -v "${repo_root}:/w" -w "/w/${dir}" \
          -e HEAP_PROBE_DIR=/w/.issue-601-heap -e NO_COLOR=1 \
          -e NODE_OPTIONS="--import /w/docs/research/probes/issue-601-test-memory/heap-probe.mjs" \
          "${image}" node /w/docs/research/probes/issue-601-test-memory/measure-tree.mjs --label "${name} ${image} #${rep}" -- ${cmd}
        echo "heap ${name} ${image} #${rep}: $(for f in "${heap_dir}"/*.json; do [[ -e "${f}" ]] && cat "${f}" && echo; done | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{const r=s.trim()?s.trim().split("\n").filter(Boolean).map(JSON.parse):[];const m=k=>Math.max(0,...r.map(x=>x[k]));console.log(JSON.stringify({processes:r.length,maxHeapUsedMiB:m("heapUsed"),maxHeapTotalMiB:m("heapTotal"),maxExternalMiB:m("external"),maxRssMiB:m("rss"),sumHeapUsedMiB:r.reduce((a,x)=>a+x.heapUsed,0)}))})')" | tee -a "${results}.heap"
      done
    done
  done
  rm -rf "${repo_root}/.issue-601-heap"
}

tests_mode() {
  environment
  pnpm --dir experiments/open-sheet-export install --frozen-lockfile >/dev/null
  cargo test --workspace --all-targets --locked --no-run >/dev/null 2>&1
  for threads in default 1; do
    for rep in $(seq "${reps}"); do
      if [[ "${threads}" == default ]]; then
        record "rust workspace test run #${rep}" cargo test --workspace --all-targets --locked
      else
        record "rust workspace test run --test-threads=1 #${rep}" cargo test --workspace --all-targets --locked -- --test-threads=1
      fi
    done
  done
}

case "${1:-}" in
  rust) rust_mode "${2:-4}" ;;
  tests) tests_mode ;;
  node) node_mode ;;
  *)
    echo "usage: run.sh rust <jobs> | tests | node" >&2
    exit 2
    ;;
esac

echo "== summary"
cat "${results}"
cat "${results}.heap" 2>/dev/null || true
