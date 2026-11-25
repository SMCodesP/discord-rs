#!/usr/bin/env bash
# ui-compare.sh — compare two screenshots and produce side-by-side with difference.
#
# Usage: scripts/ui-compare.sh <discord.png> <app.png> <saida.png>
#
# Requires identical dimensions (fails otherwise). Generates:
#   <saida.png>  montagem horizontal: [discord.png] [app.png] [diferença]
# Also writes compatibility outputs:
#   <prefixo>-lado-a-lado.png
#   <prefixo>-diff.png
# Prints the AE metric (absolute count of different pixels) and the %
# of different pixels. Output must always be outside the repository.
# A metadata sidecar (<saida>.meta.json) records size/checksums without
# personal data.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

DISCORD="${1:?usage: ui-compare.sh <discord.png> <app.png> <saida.png>}"
APP="${2:?usage: ui-compare.sh <discord.png> <app.png> <saida.png>}"
OUT_ARG="${3:?usage: ui-compare.sh <discord.png> <app.png> <saida.png>}"

[[ -f "$DISCORD" ]] || { echo "ui-compare: ERRO: referência não existe: $DISCORD" >&2; exit 2; }
[[ -f "$APP" ]] || { echo "ui-compare: ERRO: captura não existe: $APP" >&2; exit 2; }

# Output path: bare filenames go to default cache dir
if [[ "$OUT_ARG" == *"/"* ]]; then
  OUT="$OUT_ARG"
else
  OUT="$HOME/.cache/discord-rs-parity/app/$OUT_ARG"
fi

if [[ "$OUT" != *.png ]]; then
  OUT="${OUT}.png"
fi

OUT_DIR="$(mkdir -p "$(dirname "$OUT")" && cd "$(dirname "$OUT")" && pwd)"
CANONICAL_OUT="$OUT_DIR/$(basename "$OUT")"
if [[ "$CANONICAL_OUT" == "$REPO_DIR"* ]]; then
  echo "ui-compare: ERRO: saída não pode ser dentro do repositório: $OUT" >&2
  exit 1
fi
OUT="$CANONICAL_OUT"
META_OUT="${OUT%.png}.meta.json"

json_escape() {
  python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$1"
}

png_checksum() {
  sha256sum "$1" 2>/dev/null | awk '{print $1}' || echo ""
}

png_stddev() {
  identify -format "%[fx:standard_deviation]" "$1" 2>/dev/null || echo "0"
}

is_black_or_blank() {
  python3 - "$1" <<'PY'
import sys
sd=float(sys.argv[1])
print("1" if sd <= 0.02 else "0")
PY
}

SD_REF="$(identify -format "%wx%h" "$DISCORD" 2>/dev/null || echo "?")"
SA_REF="$(identify -format "%wx%h" "$APP" 2>/dev/null || echo "?")"
if [[ "$SD_REF" == "?" || "$SA_REF" == "?" ]]; then
  echo "ui-compare: ERRO: não foi possível ler dimensões das imagens" >&2
  exit 1
fi
if [[ "$SD_REF" != "$SA_REF" ]]; then
  echo "ui-compare: ERRO: tamanhos diferentes: discord=$DISCORD ($SD_REF) x app=$APP ($SA_REF)" >&2
  exit 1
fi

SD_D="$(png_stddev "$DISCORD")"
SA_D="$(png_stddev "$APP")"
if [[ "$(is_black_or_blank "$SD_D")" == "1" ]]; then
  echo "ui-compare: ERRO: referência preta/vazia: $DISCORD (stddev=$SD_D)" >&2
  exit 1
fi
if [[ "$(is_black_or_blank "$SA_D")" == "1" ]]; then
  echo "ui-compare: ERRO: captura preta/vazia: $APP (stddev=$SA_D)" >&2
  exit 1
fi

W="${SD_REF%x*}"; H="${SD_REF#*x}"
TOTAL=$(( W * H ))

TMP_DIFF="$(mktemp --suffix=.png)"
trap 'rm -f "$TMP_DIFF"' EXIT

AE_RAW="$(compare -metric AE "$DISCORD" "$APP" "$TMP_DIFF" 2>&1 || true)"
AE="$(echo "$AE_RAW" | grep -oE '^[0-9]+' | head -1 || echo "")"
[[ "$AE" =~ ^[0-9]+$ ]] || { echo "ui-compare: ERRO: métrica AE ilegível: $AE_RAW" >&2; exit 1; }
PCT="$(awk -v ae="$AE" -v total="$TOTAL" 'BEGIN { printf "%.4f", (total > 0 ? 100.0 * ae / total : 0) }')"

MAGICK_CMD="convert"
if command -v magick >/dev/null 2>&1; then
  MAGICK_CMD="magick"
fi

"$MAGICK_CMD" "$DISCORD" "$APP" "$TMP_DIFF" +append "$OUT" \
  || { echo "ui-compare: ERRO: montagem falhou" >&2; exit 1; }

PREFIX="${OUT%.png}"
cp -f "$TMP_DIFF" "${PREFIX}-diff.png" 2>/dev/null || true
"$MAGICK_CMD" "$DISCORD" "$APP" +append "${PREFIX}-lado-a-lado.png" 2>/dev/null || true

DISCORD_SHA="$(png_checksum "$DISCORD")"
APP_SHA="$(png_checksum "$APP")"
DIFF_SHA="$(png_checksum "${PREFIX}-diff.png")"
OUT_SHA="$(png_checksum "$OUT")"

cat > "$META_OUT" <<JSON
{
  "schema": "ui-compare-meta-v1",
  "status": "ok",
  "size": {"width": $W, "height": $H},
  "ae_pixels": $AE,
  "ae_percent": "$PCT",
  "reference": {"path": $(json_escape "$DISCORD"), "sha256": $(json_escape "$DISCORD_SHA"), "stddev": $(json_escape "$SD_D")},
  "capture": {"path": $(json_escape "$APP"), "sha256": $(json_escape "$APP_SHA"), "stddev": $(json_escape "$SA_D")},
  "diff": {"path": $(json_escape "${PREFIX}-diff.png"), "sha256": $(json_escape "$DIFF_SHA")},
  "montage": {"path": $(json_escape "$OUT"), "sha256": $(json_escape "$OUT_SHA")}
}
JSON

echo "ui-compare: discord=$DISCORD app=$APP size=$SD_REF"
echo "ui-compare: AE=$AE pixels diferentes de $TOTAL ($PCT%)"
echo "ui-compare: montagem: $OUT"
echo "ui-compare: lado-a-lado: ${PREFIX}-lado-a-lado.png"
echo "ui-compare: diff: ${PREFIX}-diff.png"
echo "ui-compare: META $META_OUT"
