#!/usr/bin/env bash
# Proposed replacement for scripts/experimental-designer-client-smoke.sh.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
consumer_dir="${repo_root}/examples/experimental-designer-client"
vendor_dir="${consumer_dir}/vendor/tachiko"
check_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-experimental-client-smoke.XXXXXX")"

rm -rf -- "${consumer_dir}/vendor"
cleanup() {
  rm -rf -- "${consumer_dir}/vendor" "${check_dir}"
}
trap cleanup EXIT

require_no_matches() {
  local boundary="$1"
  local status
  shift

  if "$@"; then
    echo "experimental-designer-client-smoke: ${boundary}" >&2
    exit 1
  else
    status=$?
  fi

  if [[ ${status} -ne 1 ]]; then
    echo "experimental-designer-client-smoke: source-boundary scan failed (rg exit ${status})" >&2
    exit 1
  fi
}

bash "${repo_root}/scripts/export-experimental-designer-client.sh" "${vendor_dir}"
bash "${repo_root}/scripts/export-experimental-designer-client.sh" "${check_dir}/kit"
diff -qr "${vendor_dir}" "${check_dir}/kit"

expected_files=$'README.md\nartifact-manifest.json\ndesigner_runtime.wasm\nexperimental-client.d.ts\nexperimental-client.js\nexperimental-client.worker.d.ts\nexperimental-client.worker.js\nhost/project-transfer.d.ts\nhost/project-transfer.js\nnotices/LICENSE-APACHE\nnotices/LICENSE-MIT\nnotices/THIRD_PARTY_LICENSES.md\npackage.json\nruntime/client.d.ts\nruntime/client.js\nruntime/interop-protocol.d.ts\nruntime/interop-protocol.js\nruntime/protocol.d.ts\nruntime/protocol.js\nruntime/wasm-bridge.d.ts\nruntime/wasm-bridge.js\nruntime/worker-client.d.ts\nruntime/worker-client.js\nruntime/worker-runtime.d.ts\nruntime/worker-runtime.js'
actual_files="$(find "${vendor_dir}" -type f -print | sed "s#${vendor_dir}/##" | LC_ALL=C sort)"
if [[ "${actual_files}" != "${expected_files}" ]]; then
  echo "experimental-designer-client-smoke: exported kit shape changed unexpectedly" >&2
  diff -u <(printf '%s\n' "${expected_files}") <(printf '%s\n' "${actual_files}") || true
  exit 1
fi

# diff -qr proves two exports match. This independently verifies that one
# export names every artifact byte and notice, while excluding only the
# manifest from its own digest list.
pnpm --dir "${repo_root}/apps/designer" exec node --input-type=module --eval '
  import assert from "node:assert/strict";
  import {createHash} from "node:crypto";
  import {lstat,readFile,readdir} from "node:fs/promises";
  import path from "node:path";
  const root=process.argv[1];
  const manifest=JSON.parse(await readFile(path.join(root,"artifact-manifest.json"),"utf8"));
  assert.equal(manifest.sourceRepository,"nurockplayer/tachiko-work");
  assert.match(manifest.sourceCommit,/^[a-f0-9]{40}$/);
  assert.equal(manifest.stability,"experimental");
  assert.equal(manifest.entry,"experimental-client.js");
  assert.ok(Array.isArray(manifest.files)&&manifest.files.length>0);
  assert.deepEqual(manifest.licenseNotices,["notices/LICENSE-APACHE","notices/LICENSE-MIT","notices/THIRD_PARTY_LICENSES.md"]);
  const declared=new Map();
  for(const file of manifest.files){
    assert.equal(typeof file.path,"string");
    assert.match(file.sha256,/^[a-f0-9]{64}$/);
    assert.ok(!declared.has(file.path),`duplicate manifest asset: ${file.path}`);
    declared.set(file.path,file.sha256);
    const absolute=path.join(root,file.path);
    assert.equal((await lstat(absolute)).isSymbolicLink(),false,`symlinked asset: ${file.path}`);
    assert.equal(createHash("sha256").update(await readFile(absolute)).digest("hex"),file.sha256,`digest mismatch: ${file.path}`);
  }
  const actual=[];
  async function walk(directory,prefix=""){
    for(const item of await readdir(directory,{withFileTypes:true})){
      assert.equal(item.isSymbolicLink(),false,`symlink in artifact: ${prefix}${item.name}`);
      const relative=prefix+item.name;
      if(item.isDirectory()) await walk(path.join(directory,item.name),relative+"/");
      else actual.push(relative);
    }
  }
  await walk(root);
  assert.deepEqual(actual.filter(file=>file!=="artifact-manifest.json").sort(),[...declared.keys()].sort());
  for(const notice of manifest.licenseNotices) assert.ok(declared.has(notice),`undeclared notice: ${notice}`);
' "${vendor_dir}"

require_no_matches "consumer imports private Designer source" \
  rg -n 'apps/designer|src/runtime|src/host' "${consumer_dir}/src"
require_no_matches "emitted JavaScript retains source-only imports" \
  rg -n '\.ts"' "${vendor_dir}" -g '*.js'
pnpm --dir "${repo_root}/apps/designer" exec node \
  --eval 'const manifest = JSON.parse(require("node:fs").readFileSync(process.argv[1], "utf8")); if (manifest.private !== true || manifest.packageManager !== "pnpm@11.25.0") process.exit(1);' \
  "${vendor_dir}/package.json"

pnpm --dir "${repo_root}/apps/designer" exec tsc \
  --project "${consumer_dir}/tsconfig.json" \
  --noEmit \
  --pretty false
pnpm --dir "${repo_root}/apps/designer" exec playwright test \
  --config playwright.experimental-client.config.ts

echo "experimental Designer client smoke passed"
