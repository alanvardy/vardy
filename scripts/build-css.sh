#!/usr/bin/env bash
# Compile css/site.css into the committed static/site.css with the pinned
# Tailwind standalone CLI. Binary is cached under target/ (already gitignored).
set -euo pipefail

TAILWIND_VERSION="v4.3.3"
MACOS_ARM64_SHA256="cdf646702987a743464dff4d9c60fd4480d1c1e73dd819a9a67f1078815dce9d"
LINUX_X64_SHA256="dc61b3ac6b8c9ca874c0cc4c57b2409791a64c5540404ca5f5367360babc313a"

case "$(uname -s)/$(uname -m)" in
    Darwin/arm64)
        asset="tailwindcss-macos-arm64"
        expected="$MACOS_ARM64_SHA256"
        checksum_cmd="shasum -a 256" ;;
    Linux/x86_64)
        asset="tailwindcss-linux-x64"
        expected="$LINUX_X64_SHA256"
        checksum_cmd="sha256sum" ;;
    *)
        echo "Unsupported platform: $(uname -s)/$(uname -m)" >&2; exit 1 ;;
esac

bin_dir="target/tailwindcss-cli"
mkdir -p "$bin_dir"
bin="$bin_dir/tailwindcss"

# Re-download if missing OR cached binary doesn't match the pinned checksum
# (e.g. after a version bump).
if [ ! -x "$bin" ] ||
    ! printf '%s  %s\n' "$expected" "$bin" | $checksum_cmd -c - >/dev/null 2>&1; then
    echo "⬇️  DOWNLOAD tailwindcss $TAILWIND_VERSION ($asset)"
    curl -fsSL --connect-timeout 30 --max-time 120 -o "$bin" \
        "https://github.com/tailwindlabs/tailwindcss/releases/download/${TAILWIND_VERSION}/${asset}"
    chmod +x "$bin"
fi

printf '%s  %s\n' "$expected" "$bin" | $checksum_cmd -c -
"$bin" -i css/site.css -o static/site.css --minify
echo "✅  static/site.css rebuilt"
