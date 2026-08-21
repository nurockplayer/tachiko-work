#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fail=0

extract_md_links() {
  local file="$1"
  perl -ne '
    while ( /\[[^\]]+\]\(([^)]+)\)/g ) {
      my $link = $1;
      next if $link =~ m#^(?:https?|mailto):#;
      next if $link =~ /^#/;
      $link =~ s/#.*$//;
      print "$link\n" if $link =~ /\.md$/;
    }
  ' "$file" | sort -u
}

check_missing_index_entries() {
  local index_file="$1"
  local label="$2"
  local -A links=()
  local dir="$repo_root/$(dirname "$index_file")"

  while IFS= read -r link; do
    links["$link"]=1
  done < <(extract_md_links "$repo_root/$index_file")

  local missing=0
  for path in "$dir"/*.md; do
    filename="$(basename "$path")"
    [[ "$filename" == "README.md" ]] && continue
    if [[ -z "${links["$filename"]+x}" ]]; then
      echo "MISSING $label entry: $filename"
      fail=1
      missing=1
    fi
  done

  if ((missing == 0)); then
    echo "PASS $label has complete file coverage."
  fi
}

check_broken_markdown_links() {
  local index_file="$1"
  local dir="$repo_root/$(dirname "$index_file")"

  while IFS= read -r link; do
    [[ -z "$link" ]] && continue
    local target="$dir/$link"
    if [[ ! -f "$target" ]]; then
      echo "BROKEN MARKDOWN LINK in $index_file: $link"
      fail=1
    fi
  done < <(extract_md_links "$repo_root/$index_file")
}

check_missing_index_entries "docs/decisions/README.md" "decisions"
check_missing_index_entries "docs/specs/README.md" "specs"
check_missing_index_entries "docs/architecture/README.md" "architecture"

check_broken_markdown_links "docs/README.md"
check_broken_markdown_links "docs/decisions/README.md"
check_broken_markdown_links "docs/specs/README.md"
check_broken_markdown_links "docs/architecture/README.md"

if ((fail != 0)); then
  echo "docs consistency check failed"
  exit 1
fi

echo "docs consistency check passed"
