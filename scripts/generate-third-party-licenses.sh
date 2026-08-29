#!/usr/bin/env bash
set -euo pipefail

fail() {
  echo "generate-third-party-licenses: $*" >&2
  exit 1
}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
# shellcheck source=scripts/release-lib.sh
source "${script_dir}/release-lib.sh"

command -v rustup >/dev/null 2>&1 ||
  fail "rustup is required; install the stable toolchain before generating notices"
rustup run stable rustc --version >/dev/null 2>&1 ||
  fail "the stable toolchain is not installed; run 'rustup toolchain install stable'"

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-third-party-licenses.XXXXXX")"
cleanup() {
  rm -rf -- "${work_dir}"
}
trap cleanup EXIT

vendor_dir="${work_dir}/vendor"
designer_vendor_dir="${work_dir}/designer-vendor"
tree_inventory="${work_dir}/tree-inventory.txt"
lock_inventory="${work_dir}/lock-inventory.txt"
package_inventory="${work_dir}/package-inventory.txt"
license_map="${work_dir}/license-map.txt"
license_text_dir="${work_dir}/license-texts"
mkdir "${license_text_dir}"

cd "${repo_root}"

# Cargo's vendored, checksum-verified sources are the authority for the exact
# notice text. The locked all-target normal dependency tree is the authority
# for which packages can contribute code to the tachiko CLI. --no-dedupe keeps
# Cargo from appending display-only "(*)" markers that obscure package IDs.
vendor_log="${work_dir}/cargo-vendor.log"
if ! cargo +stable vendor --locked --versioned-dirs "${vendor_dir}" >/dev/null 2>"${vendor_log}"; then
  cat "${vendor_log}" >&2
  fail "cargo vendor failed"
fi
designer_vendor_log="${work_dir}/designer-cargo-vendor.log"
if ! cargo +stable vendor \
  --manifest-path apps/designer/runtime/Cargo.toml \
  --locked \
  --versioned-dirs \
  "${designer_vendor_dir}" >/dev/null 2>"${designer_vendor_log}"; then
  cat "${designer_vendor_log}" >&2
  fail "Designer cargo vendor failed"
fi
{
  cargo +stable tree \
    -p tachiko-cli \
    --edges normal \
    --target all \
    --locked \
    --no-dedupe \
    --prefix none \
    --format '{p}|{l}|{r}'
  cargo +stable tree \
    --manifest-path apps/designer/runtime/Cargo.toml \
    -p tachiko-designer-runtime \
    --edges normal \
    --target all \
    --locked \
    --no-dedupe \
    --prefix none \
    --format '{p}|{l}|{r}'
} |
  awk -F '|' '
    {
      package = $1
      sub(/ \(proc-macro\)$/, "", package)

      # Workspace packages carry a path suffix in {p}; cargo vendor excludes
      # them and Tachiko ships its own license texts separately.
      if (package ~ / \(/) {
        next
      }

      count = split(package, parts, " ")
      if (count != 2 || parts[2] !~ /^v/) {
        print "generate-third-party-licenses: could not parse cargo tree package: " package > "/dev/stderr"
        exit 1
      }

      version = parts[2]
      sub(/^v/, "", version)
      print parts[1] "|" version "|" $2 "|" $3
    }
  ' |
  LC_ALL=C sort -u >"${tree_inventory}"

[[ -s "${tree_inventory}" ]] || fail "the shipped runtime dependency inventory is empty"

# Cargo.lock retains the immutable registry or Git source for each package.
# Fail on an ambiguous name/version pair rather than silently attributing the
# wrong source should Cargo ever permit two such packages in this lockfile.
awk '
  function quoted_value(line, value) {
    value = line
    sub(/^[^"]*"/, "", value)
    sub(/".*/, "", value)
    return value
  }
  function emit_package(key) {
    if (name == "" || version == "" || source == "") {
      return
    }
    key = name SUBSEP version
    if (seen[key] && sources[key] != source) {
      print "generate-third-party-licenses: ambiguous locked source for " name " " version > "/dev/stderr"
      exit 1
    }
    seen[key] = 1
    sources[key] = source
    print name "|" version "|" source
  }
  /^\[\[package\]\]$/ {
    emit_package()
    name = ""
    version = ""
    source = ""
    next
  }
  /^name = "/ {
    name = quoted_value($0)
    next
  }
  /^version = "/ {
    version = quoted_value($0)
    next
  }
  /^source = "/ {
    source = quoted_value($0)
    next
  }
  END {
    emit_package()
  }
' Cargo.lock apps/designer/runtime/Cargo.lock | LC_ALL=C sort -u >"${lock_inventory}"

awk -F '|' '
  NR == FNR {
    source[$1 SUBSEP $2] = $3
    next
  }
  {
    key = $1 SUBSEP $2
    if (!(key in source)) {
      print "generate-third-party-licenses: cargo tree package is missing a locked source: " $1 " " $2 > "/dev/stderr"
      failed = 1
      next
    }
    print $1 "|" $2 "|" $3 "|" source[key] "|" $4
  }
  END {
    if (failed) {
      exit 1
    }
  }
' "${lock_inventory}" "${tree_inventory}" >"${package_inventory}"

while IFS='|' read -r package version _license _source _repository; do
  [[ "${package}" =~ ^[A-Za-z0-9_-]+$ ]] || fail "unsafe package name in inventory: ${package}"
  [[ "${version}" =~ ^[0-9A-Za-z.+-]+$ ]] || fail "unsafe package version in inventory: ${version}"

  package_dir="${vendor_dir}/${package}-${version}"
  if [[ ! -d "${package_dir}" ]]; then
    package_dir="${designer_vendor_dir}/${package}-${version}"
  fi
  [[ -d "${package_dir}" ]] ||
    fail "locked runtime package was not found in cargo vendor output: ${package} ${version}"

  package_license_list="${work_dir}/licenses-${package}-${version}.txt"
  find -L "${package_dir}" -type f \
    \( -iname 'LICENSE*' -o -iname 'COPYING*' -o -iname 'NOTICE*' -o \
    -iname 'UNLICENSE*' -o -iname 'COPYRIGHT*' \) -print |
    LC_ALL=C sort >"${package_license_list}"

  [[ -s "${package_license_list}" ]] ||
    fail "${package} ${version} has no vendored LICENSE, COPYING, NOTICE, UNLICENSE, or COPYRIGHT file"

  while IFS= read -r license_file; do
    relative_file="${license_file#"${package_dir}/"}"
    [[ "${relative_file}" != "${license_file}" ]] ||
      fail "license path escaped its vendored package: ${license_file}"
    digest="$(tachiko_sha256_digest "${license_file}")" || exit 1
    [[ "${digest}" =~ ^[0-9a-fA-F]{64}$ ]] || fail "invalid SHA-256 for ${package} ${relative_file}"
    digest="$(printf '%s' "${digest}" | tr '[:upper:]' '[:lower:]')"

    stored_text="${license_text_dir}/${digest}"
    if [[ -f "${stored_text}" ]]; then
      cmp -s "${stored_text}" "${license_file}" || fail "SHA-256 collision while deduplicating license texts"
    else
      cp "${license_file}" "${stored_text}"
    fi
    printf '%s|%s|%s|%s\n' "${package}" "${version}" "${relative_file}" "${digest}" >>"${license_map}"
  done <"${package_license_list}"
done <"${package_inventory}"

LC_ALL=C sort -u "${license_map}" -o "${license_map}"

cat <<'EOF'
<!-- Generated by scripts/generate-third-party-licenses.sh. DO NOT EDIT. -->

# Third-Party Licenses

This file inventories the locked, all-target normal dependency closure of the
`tachiko` CLI and first-party Web Designer runtime. Package metadata comes from
Cargo's dependency graphs and lockfiles; license and notice text comes
byte-for-byte from `cargo vendor`.
Regenerate it with `bash scripts/generate-third-party-licenses.sh`.

## Package inventory

| Package | Version | Declared license | Locked source | Repository |
| --- | --- | --- | --- | --- |
EOF

while IFS='|' read -r package version license source repository; do
  [[ -n "${license}" ]] || license="Not declared in Cargo metadata"
  [[ -n "${repository}" ]] || repository="Not declared in Cargo metadata"
  # The literal backticks are Markdown delimiters, not shell substitutions.
  # shellcheck disable=SC2016
  printf '| `%s` | `%s` | %s | `%s` | %s |\n' \
    "${package}" "${version}" "${license}" "${source}" "${repository}"
done <"${package_inventory}"

cat <<'EOF'

## License-file map

Each entry maps a vendored package file to the exact text identified by its
SHA-256 heading below. Identical files are stored once while every package/file
attribution is retained.

| Package | Vendored file | License-text SHA-256 |
| --- | --- | --- |
EOF

while IFS='|' read -r package version relative_file digest; do
  # shellcheck disable=SC2016
  printf '| `%s %s` | `%s` | `%s` |\n' "${package}" "${version}" "${relative_file}" "${digest}"
done <"${license_map}"

printf '\n## Exact license and notice texts\n\n'
find "${license_text_dir}" -type f -print |
  sed 's|.*/||' |
  LC_ALL=C sort |
  while IFS= read -r digest; do
    # shellcheck disable=SC2016
    printf '### `%s`\n\n' "${digest}"
    printf 'Used by:\n\n'
    awk -F '|' -v digest="${digest}" '
      $4 == digest {
        printf "- `%s %s — %s`\n", $1, $2, $3
      }
    ' "${license_map}"
    printf '\n````text\n'
    cat "${license_text_dir}/${digest}"
    printf '\n````\n\n'
  done
