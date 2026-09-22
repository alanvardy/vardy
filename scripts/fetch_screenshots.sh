#!/bin/bash
set -euo pipefail
# Fetch assets attached to a Linear ticket into the repo.
# Usage: scripts/fetch_screenshots.sh <ticket-id> [dest-dir]
# Env:   MAXDIM (default 2000) — max pixel dimension passed to sips -Z
# See the ticket-screenshot-assets skill.

if [ $# -lt 1 ]; then
  echo "usage: scripts/fetch_screenshots.sh <ticket-id> [dest-dir]" >&2
  exit 2
fi

ID="$1"
DEST="${2:-static/screenshots}"
MAXDIM="${MAXDIM:-2000}"

token=$(linear auth token)
mkdir -p "$DEST"

urls=$(linear issue view "$ID" --json | python3 -c '
import json, re, sys
text = json.dumps(json.load(sys.stdin))
for u in sorted(set(re.findall(r"https://uploads\.linear\.app/[^\s\"\\\\]+", text))):
    print(u)
')

if [ -z "$urls" ]; then
  echo "no uploads.linear.app URLs found on $ID" >&2
  exit 1
fi

while read -r url; do
  [ -n "$url" ] || continue
  base=$(basename "${url%%\?*}")
  out="$DEST/$base"
  # Auth header is the raw token — a "Bearer " prefix returns 401.
  ct=$(curl -fsSL -H "Authorization: $token" -o "$out" -w '%{content_type}' "$url")
  case "$ct" in
    image/*)
      ext="${ct#image/}"
      ext="${ext%%;*}"
      ext="${ext%%+*}"
      if [ "${out##*.}" = "$out" ]; then
        mv "$out" "$out.$ext"
        out="$out.$ext"
      fi
      ;;
  esac
  sips -Z "$MAXDIM" "$out" >/dev/null
  echo "fetched $out"
done <<<"$urls"