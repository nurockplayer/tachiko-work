#!/usr/bin/env bash
set -euo pipefail

fail() {
  echo "export-experimental-designer-client: $*" >&2
  exit 1
}

if [[ "$#" -ne 1 ]]; then
  fail "usage: bash scripts/export-experimental-designer-client.sh OUTPUT_DIRECTORY"
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
designer_dir="${repo_root}/apps/designer"
output_arg="$1"
if [[ "${output_arg}" == /* ]]; then
  output_dir="${output_arg}"
else
  output_dir="${PWD}/${output_arg}"
fi
output_parent="$(dirname "${output_dir}")"
output_name="$(basename "${output_dir}")"
[[ "${output_name}" != "." && "${output_name}" != ".." ]] ||
  fail "output must name a dedicated kit directory"

if [[ -e "${output_dir}" ]]; then
  [[ -d "${output_dir}" ]] || fail "output exists and is not a directory: ${output_dir}"
  [[ -z "$(find "${output_dir}" -mindepth 1 -maxdepth 1 -print -quit)" ]] ||
    fail "output directory must be absent or empty: ${output_dir}"
fi

command -v pnpm >/dev/null 2>&1 || fail "pnpm 11.25.0 is required"
[[ "$(pnpm --dir "${designer_dir}" --version)" == "11.25.0" ]] ||
  fail "pnpm 11.25.0 is required"
pnpm --dir "${designer_dir}" install --frozen-lockfile

mkdir -p "${output_parent}"
work_dir="$(mktemp -d "${output_parent}/.tachiko-experimental-client.XXXXXX")"
cleanup() {
  rm -rf -- "${work_dir}"
}
trap cleanup EXIT
kit_dir="${work_dir}/kit"
mkdir "${kit_dir}"

bash "${repo_root}/scripts/designer-runtime-build.sh"
pnpm --dir "${designer_dir}" exec tsc \
  --project tsconfig.experimental-client.json \
  --outDir "${kit_dir}" \
  --pretty false
cp "${designer_dir}/public/designer_runtime.wasm" "${kit_dir}/designer_runtime.wasm"
cp "${designer_dir}/experimental-client-kit/README.md" "${kit_dir}/README.md"
cp "${designer_dir}/experimental-client-kit/package.json" "${kit_dir}/package.json"

if [[ -d "${output_dir}" ]]; then
  rmdir "${output_dir}"
fi
mv "${kit_dir}" "${output_dir}"
trap - EXIT
rm -rf -- "${work_dir}"

echo "experimental Designer client kit exported to ${output_dir}"
