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
# with ImageMagick `import -window <id>`, and FAILS unless the final PNG has
# exactly the requested size and is not blank. The app process and Xvfb are
# always killed at the end (no orphans).
#
# Env overrides:
#   UI_SHOT_DISPLAY      reuse an existing display (e.g. :0) instead of Xvfb.
#                        WARNING: tiling compositors (COSMIC) retile windows,
#                        so the exact-size check will very likely FAIL there.
#   UI_SHOT_CARGO_LOCK   flock lock for cargo (default /tmp/discord-rs-cargo-C.lock)
#   UI_SHOT_XVFB_DISPLAY preferred Xvfb display (default :99)
set -euo pipefail

SCREEN="${1:?usage: ui-shot.sh <tela> <saida.png> [LARGURAxALTURA]}"
OUT_ARG="${2:?usage: ui-shot.sh <tela> <saida.png> [LARGURAxALTURA]}"
SIZE="${3:-1280x800}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
FIXTURE="$REPO_DIR/tests/fixtures/ui_shot/$SCREEN.json"
LOCK="${UI_SHOT_CARGO_LOCK:-/tmp/discord-rs-cargo-C.lock}"
BIN="$REPO_DIR/target/debug/examples/ui_shot"

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
if ! [[ "$W" =~ ^[0-9]+$ && "$H" =~ ^[0-9]+$ ]]; then
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

APP_PID=""
XVFB_PID=""
CAP_DISPLAY=""
SANDBOX_XDG=""

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
}
trap cleanup EXIT

echo "ui-shot: tela=$SCREEN size=${W}x${H} out=$OUT"

# 1. Build the capture example (offline fixture loader, not production code).
echo "ui-shot: compilando examples/ui_shot..."
flock "$LOCK" timeout 900 cargo build -j 4 --example ui_shot --manifest-path "$REPO_DIR/Cargo.toml"
[[ -x "$BIN" ]] || { echo "ui-shot: ERRO: binário não gerado: $BIN" >&2; exit 1; }

# 2. Display: private Xvfb unless the caller forces one.
if [[ -n "${UI_SHOT_DISPLAY:-}" ]]; then
  CAP_DISPLAY="$UI_SHOT_DISPLAY"
  echo "ui-shot: usando display existente $CAP_DISPLAY (sem Xvfb; tamanho exato pode falhar sob tiling)"
else
  CAP_DISPLAY="${UI_SHOT_XVFB_DISPLAY:-:99}"
  for _ in $(seq 1 10); do
    if [[ ! -e "/tmp/.X11-unix/X${CAP_DISPLAY#:}" ]]; then break; fi
    CAP_DISPLAY=":${RANDOM}"
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
    || { echo "ui-shot: ERRO: Xvfb não responde em $CAP_DISPLAY" >&2; exit 1; }
  # The display may belong to a stale/foreign Xvfb (shared machine): require
  # room for the requested window, else fail instead of a clamped capture.
  DSIZE="$(DISPLAY="$CAP_DISPLAY" xdpyinfo 2>/dev/null | grep -m1 dimensions | grep -m1 -oE '[0-9]+x[0-9]+' || echo 0x0)"
  DW="${DSIZE%x*}"; DH="${DSIZE#*x}"
  if [[ "$DW" -lt "$W" || "$DH" -lt "$H" ]]; then
    echo "ui-shot: ERRO: display $CAP_DISPLAY tem ${DW}x${DH}, menor que o pedido ${W}x${H} (Xvfb de outro dono?)" >&2
    exit 1
  fi
fi

# 3. Launch the app with a unique window title.
TITLE="uishot-$SCREEN-$$-$(date +%s%N)"
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
  kill -0 "$APP_PID" 2>/dev/null || {
    echo "ui-shot: ERRO: o app morreu antes de abrir a janela. Log:" >&2
    cat /tmp/uishot-app.log >&2 || true
    exit 1
  }
  WIN="$(DISPLAY="$CAP_DISPLAY" xdotool search --name "$TITLE" 2>/dev/null | head -1 || true)"
  [[ -n "$WIN" ]] && break
  sleep 0.25
done
[[ -n "$WIN" ]] || { echo "ui-shot: ERRO: janela '$TITLE' não apareceu em 30s" >&2; cat /tmp/uishot-app.log >&2 || true; exit 1; }
echo "ui-shot: janela id=$WIN"

# 5. Enforce exact size, then wait for the first rendered (non-blank) frame.
DISPLAY="$CAP_DISPLAY" xdotool windowsize "$WIN" "$W" "$H" >/dev/null 2>&1 || true
sleep 1
TMPSHOT="$(mktemp --suffix=.png)"
READY=0
for _ in $(seq 1 30); do
  DISPLAY="$CAP_DISPLAY" import -window "$WIN" "$TMPSHOT" 2>/dev/null || true
  if [[ -s "$TMPSHOT" ]]; then
    SD="$(identify -format "%[fx:standard_deviation]" "$TMPSHOT" 2>/dev/null || echo 0)"
    NONBLANK="$(printf '%s > 0.02\n' "$SD" | bc -l 2>/dev/null || echo 0)"
    if [[ "$NONBLANK" == "1" ]]; then READY=1; break; fi
  fi
  kill -0 "$APP_PID" 2>/dev/null || {
    echo "ui-shot: ERRO: o app morreu durante a espera do primeiro frame" >&2
    cat /tmp/uishot-app.log >&2 || true
    exit 1
  }
  sleep 0.5
done
[[ "$READY" == "1" ]] || { echo "ui-shot: ERRO: nenhum frame desenhado em ~15s (janela preta/vazia?)" >&2; exit 1; }
echo "ui-shot: primeiro frame desenhado"

# 6. Final capture + strict size check.
DISPLAY="$CAP_DISPLAY" import -window "$WIN" "$OUT" \
  || { echo "ui-shot: ERRO: import falhou" >&2; exit 1; }
ACTUAL="$(identify -format "%wx%h" "$OUT" 2>/dev/null || echo "?")"
if [[ "$ACTUAL" != "${W}x${H}" ]]; then
  echo "ui-shot: ERRO: PNG final tem ${ACTUAL}, pedido ${W}x${H} (compositor redimensionou? use Xvfb padrão)" >&2
  exit 1
fi
SD="$(identify -format "%[fx:standard_deviation]" "$OUT" 2>/dev/null || echo 0)"
NONBLANK="$(printf '%s > 0.02\n' "$SD" | bc -l 2>/dev/null || echo 0)"
if [[ "$NONBLANK" != "1" ]]; then
  echo "ui-shot: ERRO: PNG final em branco/vazio (stddev=$SD)" >&2
  exit 1
fi
rm -f "$TMPSHOT"

echo "ui-shot: OK $OUT (${ACTUAL}, stddev=$SD)"
