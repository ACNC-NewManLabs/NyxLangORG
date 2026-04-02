#!/usr/bin/env bash
set -euo pipefail

out_iso="${1:-build/alpine.iso}"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

base_url="https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86/"

index_html="$tmp_dir/index.html"
curl -fsSL "$base_url" -o "$index_html"

# Pick the highest version alpine-virt-*-x86.iso (small) or fall back to alpine-extended.
iso_name="$(
  rg -o 'alpine-virt-[0-9]+\.[0-9]+\.[0-9]+-x86\.iso' "$index_html" \
    | sort -Vu \
    | tail -n 1
)"

if [[ -z "${iso_name:-}" ]]; then
  iso_name="$(
    rg -o 'alpine-extended-[0-9]+\.[0-9]+\.[0-9]+-x86\.iso' "$index_html" \
      | sort -Vu \
      | tail -n 1
  )"
fi

if [[ -z "${iso_name:-}" ]]; then
  echo "error: could not find Alpine x86 ISO on $base_url" >&2
  exit 1
fi

mkdir -p "$(dirname "$out_iso")"
echo "Fetching: $iso_name"
curl -fSL "$base_url$iso_name" -o "$out_iso"
echo "OK: wrote $out_iso"
