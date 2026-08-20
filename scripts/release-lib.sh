#!/usr/bin/env bash

# Small shared policy surface for release scripts. Keep release target and
# payload decisions here so packaging, verification, and local checks cannot
# silently drift apart.

tachiko_workspace_version() {
  local repo_root="$1"
  local version

  version="$(awk '
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
  ' "${repo_root}/Cargo.toml")"

  if [[ ! "${version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]]; then
    echo "release-lib: could not derive a valid workspace version from Cargo.toml" >&2
    return 1
  fi

  printf '%s\n' "${version}"
}

tachiko_supported_target() {
  case "$1" in
    x86_64-unknown-linux-gnu | aarch64-apple-darwin | x86_64-apple-darwin | x86_64-pc-windows-msvc) return 0 ;;
    *) return 1 ;;
  esac
}

tachiko_executable_name() {
  if [[ "$1" == *-windows-* ]]; then
    printf '%s\n' "tachiko.exe"
  else
    printf '%s\n' "tachiko"
  fi
}

tachiko_release_payloads() {
  printf '%s\n' README.md CHANGELOG.md LICENSE-APACHE LICENSE-MIT THIRD_PARTY_LICENSES.md
}

tachiko_sha256_digest() {
  local file="$1"

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${file}" | awk '{ print $1 }'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "${file}" | awk '{ print $1 }'
  else
    echo "release-lib: no SHA-256 tool found; install sha256sum or shasum" >&2
    return 1
  fi
}
