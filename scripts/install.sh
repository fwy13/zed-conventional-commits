#!/usr/bin/env bash
# Install a prebuilt zed-cc GitHub Release and register it with all Zed projects.
# Usage: bash scripts/install.sh OWNER/REPOSITORY
set -euo pipefail

if [[ $# -ne 1 || ! "$1" =~ ^[a-zA-Z0-9_.-]+/[a-zA-Z0-9_.-]+$ ]]; then
  echo "Usage: bash scripts/install.sh GITHUB_OWNER/REPOSITORY" >&2
  exit 2
fi
repo="$1"
os="$(uname -s)"
arch="$(uname -m)"
case "${os}:${arch}" in
  Linux:x86_64) target=x86_64-unknown-linux-gnu ;;
  Linux:aarch64|Linux:arm64) target=aarch64-unknown-linux-gnu ;;
  Darwin:x86_64) target=x86_64-apple-darwin ;;
  Darwin:arm64|Darwin:aarch64) target=aarch64-apple-darwin ;;
  *) echo "Unsupported platform ${os}/${arch}; use cargo install --git or build from source" >&2; exit 1 ;;
esac
asset="zed-cc-${target}.tar.gz"
base="https://github.com/${repo}/releases/latest/download"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
printf 'Downloading %s from %s\n' "$asset" "$repo"
curl --fail --location --silent --show-error "$base/$asset" --output "$tmp/$asset"
curl --fail --location --silent --show-error "$base/SHA256SUMS.txt" --output "$tmp/SHA256SUMS.txt"
line="$(grep -F "  $asset" "$tmp/SHA256SUMS.txt" | head -n 1 || true)"
if [[ -z "$line" ]]; then
  echo "Release checksum missing for $asset" >&2
  exit 1
fi
expected="${line%% *}"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$tmp/$asset" | awk '{print $1}')"
else
  actual="$(shasum -a 256 "$tmp/$asset" | awk '{print $1}')"
fi
if [[ "$actual" != "$expected" ]]; then
  echo "Checksum mismatch. Refusing to install." >&2
  exit 1
fi
tar -xzf "$tmp/$asset" -C "$tmp" zed-cc
install_dir="${HOME}/.local/bin"
mkdir -p "$install_dir"
install -m 755 "$tmp/zed-cc" "$install_dir/zed-cc"
"$install_dir/zed-cc" init-zed --global --bin "$install_dir/zed-cc"
echo "Installed: $install_dir/zed-cc"
echo "Restart Zed and run: task: spawn → Conventional Commit (Rust)"
echo "If your shell cannot find zed-cc, add $install_dir to PATH."
