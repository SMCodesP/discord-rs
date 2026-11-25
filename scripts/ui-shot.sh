#!/usr/bin/env bash
# ui-shot.sh — deterministic screenshot of the app in a fixture state.
#
# Usage: scripts/ui-shot.sh <tela> <saida.png> [LARGURAxALTURA]
#   <tela>    one of: login friends dm guild settings profile_popout
#                     media_picker search  (fixture tests/fixtures/ui_shot/<tela>.json)
#   <saida>   output PNG path. A bare filename is placed under
#             ~/.cache/discord-rs-parity/app/ (never inside the repo).
#   [WxH]     window size, default 1280x800.
#
# Method: builds examples/ui_shot, runs it on the X11 backend under a private
# Xvfb display (exact window size, no compositor interference), finds the
# window by its unique title, waits for the first rendered frame, captures
# with ImageMagick `import -window <id>` (fallback: `xwd -id | magick xwd:-`),
# and FAILS unless the final PNG has exactly the requested size and is not
# blank/black. The app process and Xvfb are always killed at the end.
# A metadata sidecar (<saida>.meta.json) records viewport/DPR/title/route
# without any personal data.
#
# Env overrides:
#   UI_SHOT_DISPLAY      reuse an existing display (e.g. :0) instead of Xvfb.
#                        WARNING: tiling compositors (COSMIC) retile windows,
#                        so the exact-size check will very likely FAIL there.
#   UI_SHOT_CARGO_LOCK   flock lock for cargo (default /tmp/discord-rs-cargo-B.lock)
#   UI_SHOT_XVFB_DISPLAY preferred Xvfb display (default :99)
#   UI_SHOT_CAPTURE_TIMEOUT_SECS  per-capture command timeout (default 15)
#   UI_SHOT_READY_TIMEOUT_SECS    max wait for first non-black frame (default 20)
set -euo pipefail

SCREEN="${1:?usage: ui-shot.sh <tela> <saida.png> [LARGURAxALTURA]}"
OUT_ARG="${2:?usage: ui-shot.sh <tela> <saida.png> [LARGURAxALTURA]}"
SIZE="${3:-1280x800}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
FIXTURE="$REPO_DIR/tests/fixtures/ui_shot/$SCREEN.json"
LOCK="${UI_SHOT_CARGO_LOCK:-/tmp/discord-rs-cargo-B.lock}"
TARGET_DIR="$REPO_DIR/target"
if [[ -L "$TARGET_DIR" ]]; then
  RESOLVED_TARGET="$(readlink -f "$TARGET_DIR")"
  BIN="$TARGET_DIR/debug/examples/ui_shot"
else
  RESOLVED_TARGET="$TARGET_DIR"
  BIN="$TARGET_DIR/debug/examples/ui_shot"
fi
CAPTURE_TIMEOUT="${UI_SHOT_CAPTURE_TIMEOUT_SECS:-15}"
READY_TIMEOUT="${UI_SHOT_READY_TIMEOUT_SECS:-20}"

if [[ ! -f "$FIXTURE" ]]; then
  echo "ui-shot: ERRO: fixture inexistente: $FIXTURE" >&2
  echo "ui-shot: telas válidas: $(ls "$REPO_DIR/tests/fixtures/ui_shot/" | sed 's/\.json//' | tr '\n' ' ')" >&2
  exit 2
fi
if [[ "$SIZE" != *x* ]]; then
  echo "ui-shot: ERRO: tamanho deve ser LARGURAxALTURA, veio: $SIZE" >&2
  exit 2
fi
W="${SIZE%x*}"; H="${SIZE#*x}"
if ! [[ "$W" =~ ^[0-9]+$ && "$H" =~ ^[0-9]+$ && "$W" -gt 0 && "$H" -gt 0 ]]; then
  echo "ui-shot: ERRO: tamanho inválido: $SIZE" >&2
  exit 2
fi

# Output path: bare filenames go to the out-of-repo default dir.
if [[ "$OUT_ARG" == *"/"* ]]; then
  OUT="$OUT_ARG"
else
  OUT="$HOME/.cache/discord-rs-parity/app/$OUT_ARG"
fi
mkdir -p "$(dirname "$OUT")"
OUT="$(cd "$(dirname "$OUT")" && pwd)/$(basename "$OUT")"
META_OUT="${OUT%.png}.meta.json"

# Never write captures/metadata inside the repo.
if [[ "$OUT" == "$REPO_DIR"* ]]; then
  echo "ui-shot: ERRO: saída não pode ser dentro do repositório: $OUT" >&2
  exit 1
fi

APP_PID=""
XVFB_PID=""
CAP_DISPLAY=""
SANDBOX_XDG=""
TMPSHOT=""

cleanup() {
  if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" 2>/dev/null; then
    kill "$APP_PID" 2>/dev/null || true
    for _ in $(seq 1 20); do
      kill -0 "$APP_PID" 2>/dev/null || break
      sleep 0.25
    done
    if kill -0 "$APP_PID" 2>/dev/null; then
      kill -9 "$APP_PID" 2>/dev/null || true
    fi
  fi
  if [[ -n "$XVFB_PID" ]] && kill -0 "$XVFB_PID" 2>/dev/null; then
    kill "$XVFB_PID" 2>/dev/null || true
    sleep 0.5
    kill -9 "$XVFB_PID" 2>/dev/null || true
  fi
  if [[ -n "$SANDBOX_XDG" && -d "$SANDBOX_XDG" ]]; then
    rm -rf "$SANDBOX_XDG"
  fi
  if [[ -n "$TMPSHOT" && -f "$TMPSHOT" ]]; then
    rm -f "$TMPSHOT"
  fi
}
trap cleanup EXIT

json_escape() {
  python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$1"
}

write_meta() {
  local status="$1" cause="$2" actual="$3" stddev="$4" checksum="$5"
  local dpr="${UI_SHOT_DPR:-1.0}"
  local title_json route_json cause_json actual_json stddev_json checksum_json
  title_json="$(json_escape "$TITLE")"
  route_json="$(json_escape "$SCREEN")"
  cause_json="$(json_escape "$cause")"
  actual_json="$(json_escape "$actual")"
  stddev_json="$(json_escape "$stddev")"
  checksum_json="$(json_escape "$checksum")"
  cat > "$META_OUT" <<JSON
{
  "schema": "ui-shot-meta-v1",
  "status": "$status",
  "screen": $route_json,
  "route": $route_json,
  "fixture": $route_json,
  "title": $title_json,
  "viewport": {"width": $W, "height": $H},
  "dpr": $dpr,
  "actual_size": $actual_json,
  "stddev": $stddev_json,
  "sha256": $checksum_json,
  "capture_backend": "$(json_escape "$CAPTURE_BACKEND")",
  "display": "$(json_escape "$CAP_DISPLAY")",
  "cause": $cause_json
}
JSON
}

fail() {
  local cause="$1"
  echo "ui-shot: ERRO: $cause" >&2
  write_meta "error" "$cause" "" "" ""
  exit 1
}

png_stddev() {
  local f="$1"
  identify -format "%[fx:standard_deviation]" "$f" 2>/dev/null || echo "0"
}

png_size() {
  local f="$1"
  identify -format "%wx%h" "$f" 2>/dev/null || echo "?"
}

png_checksum() {
  local f="$1"
  sha256sum "$f" 2>/dev/null | awk '{print $1}' || echo ""
}

is_black_or_blank() {
  local f="$1" sd="$2"
  # stddev very close to 0 means uniform image (all black, all white, etc.)
  python3 - "$sd" <<'PY'
import sys
sd=float(sys.argv[1])
print("1" if sd <= 0.02 else "0")
PY
}

capture_window() {
  local win="$1" out="$2"
  local tmp="${out}.tmp.$$"
  rm -f "$tmp"

  # Primary: ImageMagick import with timeout.
  if timeout "$CAPTURE_TIMEOUT" env DISPLAY="$CAP_DISPLAY" import -window "$win" "$tmp" >/tmp/uishot-capture.log 2>&1; then
    if [[ -s "$tmp" ]]; then mv -f "$tmp" "$out"; CAPTURE_BACKEND="import"; return 0; fi
  fi
  local import_rc=$?
  echo "ui-shot: captura primária (import) falhou rc=$import_rc; tentando fallback" >&2
  cat /tmp/uishot-capture.log >&2 || true
  rm -f "$tmp"

  # Fallback: xwd piped to magick/convert.
  if command -v xwd >/dev/null 2>&1; then
    if timeout "$CAPTURE_TIMEOUT" bash -lc 'xwd -silent -id "$1" -display "$2" | magick xwd:- "$3"' _ "$win" "$CAP_DISPLAY" "$tmp" >/tmp/uishot-capture.log 2>&1; then
      if [[ -s "$tmp" ]]; then mv -f "$tmp" "$out"; CAPTURE_BACKEND="xwd+magick"; return 0; fi
    fi
    local xwd_rc=$?
    echo "ui-shot: fallback (xwd+magick) falhou rc=$xwd_rc" >&2
    cat /tmp/uishot-capture.log >&2 || true
    rm -f "$tmp"
  else
    echo "ui-shot: fallback indisponível: xwd não encontrado" >&2
  fi

  CAPTURE_BACKEND="none"
  return 1
}

CAPTURE_BACKEND="unknown"
TITLE="uishot-$SCREEN-$$-$(date +%s%N)"

echo "ui-shot: tela=$SCREEN size=${W}x${H} out=$OUT"

# 1. Build the capture example (offline fixture loader, not production code).
echo "ui-shot: compilando examples/ui_shot..."
if ! flock "$LOCK" timeout 900 cargo build -j 4 --example ui_shot --manifest-path "$REPO_DIR/Cargo.toml"; then
  fail "cargo build falhou (veja saída acima)"
fi
if [[ ! -x "$BIN" && -x "$RESOLVED_TARGET/debug/examples/ui_shot" ]]; then
  BIN="$RESOLVED_TARGET/debug/examples/ui_shot"
fi
[[ -x "$BIN" ]] || fail "binário não gerado: $BIN"

# 2. Display: private Xvfb unless the caller forces one.
if [[ -n "${UI_SHOT_DISPLAY:-}" ]]; then
  CAP_DISPLAY="$UI_SHOT_DISPLAY"
  echo "ui-shot: usando display existente $CAP_DISPLAY (sem Xvfb; tamanho exato pode falhar sob tiling)"
else
  CAP_DISPLAY="${UI_SHOT_XVFB_DISPLAY:-:99}"
  for _ in $(seq 1 10); do
    if [[ ! -e "/tmp/.X11-unix/X${CAP_DISPLAY#:}" ]]; then break; fi
    CAP_DISPLAY=":$((RANDOM % 500 + 100))"
  done
  VSCREEN="$(( W + 120 ))x$(( H + 120 ))x24"
  echo "ui-shot: iniciando Xvfb em $CAP_DISPLAY ($VSCREEN)..."
  setsid Xvfb "$CAP_DISPLAY" -screen 0 "$VSCREEN" > /tmp/uishot-xvfb.log 2>&1 < /dev/null &
  XVFB_PID=$!
  for _ in $(seq 1 40); do
    DISPLAY="$CAP_DISPLAY" xdpyinfo >/dev/null 2>&1 && break
    sleep 0.25
  done
  DISPLAY="$CAP_DISPLAY" xdpyinfo >/dev/null 2>&1 \
    || fail "Xvfb não responde em $CAP_DISPLAY"
  DSIZE="$(DISPLAY="$CAP_DISPLAY" xdpyinfo 2>/dev/null | awk '/dimensions:/ {if (!seen) {print $2; seen=1}}')"
  [[ -z "$DSIZE" ]] && DSIZE="0x0"
  DW="${DSIZE%x*}"; DH="${DSIZE#*x}"
  if [[ "$DW" -lt "$W" || "$DH" -lt "$H" ]]; then
    fail "display $CAP_DISPLAY tem ${DW}x${DH}, menor que o pedido ${W}x${H}"
  fi
fi

# 3. Launch the app with a unique window title.
SANDBOX_XDG="$(mktemp -d)"
echo "ui-shot: abrindo janela (title=$TITLE)..."
XDG_CONFIG_HOME="$SANDBOX_XDG" \
  env -u WAYLAND_DISPLAY -u DISCORD_TOKEN -u DISCORD_TOKEN_IS_USER \
  DISPLAY="$CAP_DISPLAY" \
  "$BIN" "$FIXTURE" "$TITLE" "${W}x${H}" > /tmp/uishot-app.log 2>&1 &
APP_PID=$!

# 4. Wait for the window to appear.
WIN=""
for _ in $(seq 1 120); do
  if ! kill -0 "$APP_PID" 2>/dev/null; then
    cat /tmp/uishot-app.log >&2 || true
    fail "o app morreu antes de abrir a janela"
  fi
  WIN="$(DISPLAY="$CAP_DISPLAY" xdotool search --name "$TITLE" 2>/dev/null | head -1 || true)"
  [[ -n "$WIN" ]] && break
  sleep 0.25
done
[[ -n "$WIN" ]] || { cat /tmp/uishot-app.log >&2 || true; fail "janela '$TITLE' não apareceu em 30s"; }
echo "ui-shot: janela id=$WIN"

# 5. Enforce exact size, then wait for the first rendered (non-black) frame.
DISPLAY="$CAP_DISPLAY" xdotool windowsize "$WIN" "$W" "$H" >/dev/null 2>&1 || true
sleep 1
TMPSHOT="$(mktemp --suffix=.png)"
READY=0
READY_DEADLINE=$((SECONDS + READY_TIMEOUT))
while [[ "$SECONDS" -lt "$READY_DEADLINE" ]]; do
  if capture_window "$WIN" "$TMPSHOT"; then
    if [[ -s "$TMPSHOT" ]]; then
      SD="$(png_stddev "$TMPSHOT")"
      if [[ "$(is_black_or_blank "$TMPSHOT" "$SD")" != "1" ]]; then
        READY=1
        break
      fi
    fi
  else
    echo "ui-shot: tentativa de captura falhou (aguardando/retry)" >&2
  fi
  if ! kill -0 "$APP_PID" 2>/dev/null; then
    cat /tmp/uishot-app.log >&2 || true
    fail "o app morreu durante a espera do primeiro frame"
  fi
  sleep 0.5
done
[[ "$READY" == "1" ]] || fail "nenhum frame desenhado em ${READY_TIMEOUT}s (janela preta/vazia ou timeout de captura)"
echo "ui-shot: primeiro frame desenhado"

# 6. Final capture + strict checks.
if ! capture_window "$WIN" "$OUT"; then
  fail "captura final falhou (import e fallback xwd+magick)"
fi
ACTUAL="$(png_size "$OUT")"
if [[ "$ACTUAL" != "${W}x${H}" ]]; then
  fail "PNG final tem ${ACTUAL}, pedido ${W}x${H}"
fi
SD="$(png_stddev "$OUT")"
if [[ "$(is_black_or_blank "$OUT" "$SD")" == "1" ]]; then
  fail "PNG final preto/vazio (stddev=$SD)"
fi
CHECKSUM="$(png_checksum "$OUT")"
rm -f "$TMPSHOT"
TMPSHOT=""

write_meta "ok" "" "$ACTUAL" "$SD" "$CHECKSUM"
echo "ui-shot: OK $OUT (${ACTUAL}, stddev=$SD, sha256=$CHECKSUM)"
echo "ui-shot: META $META_OUT"
