#!/usr/bin/env bash
# ui-compare.sh — compare two screenshots and produce side-by-side with difference.
#
# Usage: scripts/ui-compare.sh <discord.png> <app.png> <saida.png>
#
# Requires identical dimensions (fails otherwise). Generates:
#   <saida.png>  montagem horizontal: [discord.png] [app.png] [diferença]
# Prints the AE metric (absolute count of different pixels) and the %
# of different pixels. Output must always be outside the repository.
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

# Canonical check: saída SEMPRE fora do repo
OUT_DIR="$(mkdir -p "$(dirname "$OUT")" && cd "$(dirname "$OUT")" && pwd)"
CANONICAL_OUT="$OUT_DIR/$(basename "$OUT")"
if [[ "$CANONICAL_OUT" == "$REPO_DIR"* ]]; then
  echo "ui-compare: ERRO: saída não pode ser dentro do repositório: $OUT" >&2
  exit 1
fi

SD="$(identify -format "%wx%h" "$DISCORD" 2>/dev/null || echo "?")"
SA="$(identify -format "%wx%h" "$APP" 2>/dev/null || echo "?")"
if [[ "$SD" != "$SA" ]]; then
  echo "ui-compare: ERRO: tamanhos diferentes: discord=$DISCORD ($SD) x app=$APP ($SA)" >&2
  exit 1
fi
W="${SD%x*}"; H="${SD#*x}"
TOTAL=$(( W * H ))

TMP_DIFF="$(mktemp --suffix=.png)"
trap 'rm -f "$TMP_DIFF"' EXIT

# ImageMagick compare generates the difference image and writes AE metric to stderr
AE_RAW="$(compare -metric AE "$DISCORD" "$APP" "$TMP_DIFF" 2>&1 || true)"
AE="$(echo "$AE_RAW" | grep -oE '^[0-9]+' || echo "")"
[[ "$AE" =~ ^[0-9]+$ ]] || { echo "ui-compare: ERRO: métrica AE ilegível: $AE_RAW" >&2; exit 1; }
PCT="$(awk -v ae="$AE" -v total="$TOTAL" 'BEGIN { printf "%.4f", (total > 0 ? 100.0 * ae / total : 0) }')"

MAGICK_CMD="convert"
if command -v magick >/dev/null 2>&1; then
  MAGICK_CMD="magick"
fi

# Monta as duas imagens lado a lado e uma terceira com a diferença
"$MAGICK_CMD" "$DISCORD" "$APP" "$TMP_DIFF" +append "$OUT" \
  || { echo "ui-compare: ERRO: $MAGICK_CMD +append falhou" >&2; exit 1; }

# Se prefixo for diferente de .png, gera também os arquivos auxiliares para compatibilidade
PREFIX="${OUT%.png}"
cp -f "$TMP_DIFF" "${PREFIX}-diff.png" 2>/dev/null || true
"$MAGICK_CMD" "$DISCORD" "$APP" +append "${PREFIX}-lado-a-lado.png" 2>/dev/null || true

echo "ui-compare: discord=$DISCORD app=$APP size=$SD"
echo "ui-compare: AE=$AE pixels diferentes de $TOTAL ($PCT%)"
echo "ui-compare: montagem: $OUT"
echo "ui-compare: lado-a-lado: ${PREFIX}-lado-a-lado.png"
echo "ui-compare: diff: ${PREFIX}-diff.png"
