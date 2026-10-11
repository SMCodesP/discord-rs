#!/usr/bin/env bash
# ui-compare.sh — compare two screenshots of equal size.
#
# Usage: scripts/ui-compare.sh <a.png> <b.png> <saida-prefixo>
#
# Requires identical dimensions (fails otherwise). Generates:
#   <prefixo>-lado-a-lado.png   side-by-side (a left, b right)
#   <prefixo>-diff.png           ImageMagick `compare` highlight of differing pixels
# Prints the AE metric (absolute count of different pixels) and the %
# of different pixels.
set -euo pipefail

A="${1:?usage: ui-compare.sh <a.png> <b.png> <saida-prefixo>}"
B="${2:?usage: ui-compare.sh <a.png> <b.png> <saida-prefixo>}"
PREFIX="${3:?usage: ui-compare.sh <a.png> <b.png> <saida-prefixo>}"

[[ -f "$A" ]] || { echo "ui-compare: ERRO: não existe: $A" >&2; exit 2; }
[[ -f "$B" ]] || { echo "ui-compare: ERRO: não existe: $B" >&2; exit 2; }

SA="$(identify -format "%wx%h" "$A" 2>/dev/null || echo "?")"
SB="$(identify -format "%wx%h" "$B" 2>/dev/null || echo "?")"
if [[ "$SA" != "$SB" ]]; then
  echo "ui-compare: ERRO: tamanhos diferentes: $A=$SA x $B=$SB" >&2
  exit 1
fi
W="${SA%x*}"; H="${SA#*x}"
TOTAL=$(( W * H ))

SBS="${PREFIX}-lado-a-lado.png"
DIFF="${PREFIX}-diff.png"
mkdir -p "$(dirname "$PREFIX")"

convert "$A" "$B" +append "$SBS" \
  || { echo "ui-compare: ERRO: convert +append falhou" >&2; exit 1; }

# compare exits 1 when images differ — expected; metric goes to stderr in
# IM7 as "<AE> (<normalized>)" — take the leading integer count.
AE_RAW="$(compare -metric AE "$A" "$B" "$DIFF" 2>&1 || true)"
AE="$(echo "$AE_RAW" | grep -oE '^[0-9]+' || echo "")"
[[ "$AE" =~ ^[0-9]+$ ]] || { echo "ui-compare: ERRO: métrica AE ilegível: $AE" >&2; exit 1; }
PCT="$(awk -v ae="$AE" -v total="$TOTAL" 'BEGIN { printf "%.4f", (total > 0 ? 100.0 * ae / total : 0) }')"

echo "ui-compare: a=$A b=$B size=$SA"
echo "ui-compare: AE=$AE pixels diferentes de $TOTAL ($PCT%)"
echo "ui-compare: lado-a-lado: $SBS"
echo "ui-compare: diff: $DIFF"
