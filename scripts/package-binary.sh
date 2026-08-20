#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 <target> <output-directory>" >&2
}

fail() {
  echo "package-binary: $*" >&2
  exit 1
}

workspace_version() {
  awk '
    /^\[workspace\.package\][[:space:]]*$/ {
      inside_workspace_package = 1
      next
    }
    inside_workspace_package && /^\[/ {
      exit
    }
    inside_workspace_package && /^[[:space:]]*version[[:space:]]*=/ {
      line = $0
      sub(/^[^"]*"/, "", line)
      sub(/".*/, "", line)
      print line
      exit
    }
  ' "${repo_root}/Cargo.toml"
}

sha256_digest() {
  local file="$1"

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${file}" | awk '{ print $1 }'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "${file}" | awk '{ print $1 }'
  else
    fail "no SHA-256 tool found; install sha256sum or shasum"
  fi
}

if [[ "$#" -ne 2 ]]; then
  usage
  exit 2
fi

target="$1"
output_dir="$2"

case "${target}" in
  x86_64-unknown-linux-gnu | aarch64-apple-darwin | x86_64-apple-darwin | x86_64-pc-windows-msvc) ;;
  *)
    fail "unsupported target '${target}'; expected a supported Tachiko release target"
    ;;
esac

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(workspace_version)"
[[ "${version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]] ||
  fail "could not derive a valid workspace version from Cargo.toml"

if [[ "${target}" == *-windows-* ]]; then
  executable_name="tachiko.exe"
else
  executable_name="tachiko"
fi

binary="${repo_root}/target/${target}/release/${executable_name}"
[[ -f "${binary}" ]] ||
  fail "missing release binary '${binary}'; build it with cargo build --release --locked --target ${target} -p tachiko-cli"
[[ -x "${binary}" ]] || fail "release binary is not executable: ${binary}"

for payload in README.md CHANGELOG.md LICENSE-APACHE LICENSE-MIT; do
  [[ -f "${repo_root}/${payload}" ]] || fail "missing required payload: ${payload}"
done

mkdir -p "${output_dir}"
output_dir="$(cd "${output_dir}" && pwd)"

artifact_root="tachiko-${version}-${target}"
archive_name="${artifact_root}.tar.gz"
archive_path="${output_dir}/${archive_name}"
checksum_path="${archive_path}.sha256"

[[ ! -e "${archive_path}" ]] || fail "refusing to overwrite existing archive: ${archive_path}"
[[ ! -e "${checksum_path}" ]] || fail "refusing to overwrite existing checksum: ${checksum_path}"

work_dir="$(mktemp -d "${output_dir}/.tachiko-package.XXXXXX")"
published_archive=0
published_checksum=0
completed=0

cleanup() {
  if [[ "${completed}" -ne 1 ]]; then
    if [[ "${published_checksum}" -eq 1 ]]; then
      rm -f -- "${checksum_path}"
    fi
    if [[ "${published_archive}" -eq 1 ]]; then
      rm -f -- "${archive_path}"
    fi
  fi
  rm -rf -- "${work_dir}"
}
trap cleanup EXIT

stage_root="${work_dir}/${artifact_root}"
mkdir "${stage_root}"
cp "${binary}" "${stage_root}/${executable_name}"
cp "${repo_root}/README.md" "${repo_root}/CHANGELOG.md" \
  "${repo_root}/LICENSE-APACHE" "${repo_root}/LICENSE-MIT" "${stage_root}/"
chmod 0755 "${stage_root}/${executable_name}"
chmod 0644 "${stage_root}/README.md" "${stage_root}/CHANGELOG.md" \
  "${stage_root}/LICENSE-APACHE" "${stage_root}/LICENSE-MIT"

# Fixed timestamps, ownership, order, and gzip headers make equivalent inputs
# produce equivalent archives. COPYFILE_DISABLE prevents AppleDouble entries on
# macOS; the exact file list avoids filesystem-dependent traversal order.
TZ=UTC touch -t 198001010000 \
  "${stage_root}/${executable_name}" \
  "${stage_root}/README.md" \
  "${stage_root}/CHANGELOG.md" \
  "${stage_root}/LICENSE-APACHE" \
  "${stage_root}/LICENSE-MIT" \
  "${stage_root}"

partial_archive="${work_dir}/${archive_name}.partial"
(
  cd "${work_dir}"
  COPYFILE_DISABLE=1 tar \
    --format=ustar \
    --uid 0 \
    --gid 0 \
    --uname root \
    --gname root \
    --no-recursion \
    -cf - \
    "${artifact_root}" \
    "${artifact_root}/${executable_name}" \
    "${artifact_root}/README.md" \
    "${artifact_root}/CHANGELOG.md" \
    "${artifact_root}/LICENSE-APACHE" \
    "${artifact_root}/LICENSE-MIT"
) | gzip -n >"${partial_archive}"

digest="$(sha256_digest "${partial_archive}")"
[[ "${digest}" =~ ^[0-9a-fA-F]{64}$ ]] || fail "SHA-256 tool returned an invalid digest"

partial_checksum="${work_dir}/${archive_name}.sha256.partial"
printf '%s  %s\n' "${digest}" "${archive_name}" >"${partial_checksum}"

mv "${partial_archive}" "${archive_path}"
published_archive=1
mv "${partial_checksum}" "${checksum_path}"
published_checksum=1
completed=1

printf '%s\n' "${archive_path}"
printf '%s\n' "${checksum_path}"
