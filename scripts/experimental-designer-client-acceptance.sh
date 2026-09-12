#!/usr/bin/env bash
set -euo pipefail
[[ $# -eq 2 ]] || { echo "usage: KIT_PATH EXPECTED_SOURCE_SHA" >&2; exit 64; }
kit_path="$1"
source_sha="$2"
[[ "$source_sha" =~ ^[0-9a-f]{40}$ ]] || { echo "invalid source SHA" >&2; exit 64; }
[[ -d "$kit_path" && ! -L "$kit_path" ]] || { echo "BLOCKED: complete exported kit directory is required." >&2; exit 78; }
repo_root="$(cd "$(dirname "$BASH_SOURCE")/.." && pwd)"
seed_root="$repo_root/tests/consumer-kit/seed"
driver="$repo_root/tests/consumer-kit/public-kit-storage-driver.mjs"
launcher="$repo_root/tests/consumer-kit/run-canary.mjs"
node --input-type=module - "$seed_root" <<'NODE'
import assert from "node:assert/strict"; import {createHash} from "node:crypto"; import {readFile} from "node:fs/promises"; import path from "node:path";
const root=process.argv[2], data=JSON.parse(await readFile(path.join(root,"provenance.json"),"utf8"));
assert.equal(data.seedCommit,"0bb56541d3820b1197da79aa2ce36c1051093d85");
for(const [file,digest] of Object.entries(data.files)) assert.equal(createHash("sha256").update(await readFile(path.join(root,file))).digest("hex"),digest,"seed byte drift: "+file);
console.log(JSON.stringify({case:"preserved-sheet-seed",status:"PASS",files:Object.keys(data.files).length}));
NODE
playwright_module="$(pnpm --dir "$repo_root/apps/designer" exec node --eval 'console.log(require.resolve("@playwright/test"))')" || { echo "BLOCKED: installed @playwright/test is required." >&2; exit 78; }
WORK_CLIENT_KIT="$kit_path" WORK_CORE_COMMIT="$source_sha" pnpm --dir "$repo_root/apps/designer" exec node "$seed_root/tests/kit.mjs"
WORK_CLIENT_KIT="$kit_path" WORK_PLAYWRIGHT_MODULE="$playwright_module" WORK_STORAGE_DRIVER="$driver" pnpm --dir "$repo_root/apps/designer" exec node "$seed_root/tests/storage.mjs"
WORK_CLIENT_KIT="$kit_path" WORK_PLAYWRIGHT_MODULE="$playwright_module" pnpm --dir "$repo_root/apps/designer" exec node "$launcher" "$seed_root/scripts/serve-canary.mjs" "$seed_root/tests/browser.mjs"
