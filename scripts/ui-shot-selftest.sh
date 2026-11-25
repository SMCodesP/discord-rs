#!/usr/bin/env bash
# ui-shot-selftest.sh — synthetic reliability checks for ui-shot/ui-compare.
#
# Runs without a browser/Chrome. Validates:
# - comparable pair generation with same checksum/dimensions for same fixture
# - explicit failure on black/blank image
# - explicit failure on capture timeout
# - metadata sidecars contain no personal-data keys
#
# Usage: scripts/ui-shot-selftest.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TMPROOT="$(mktemp -d /tmp/ui-shot-selftest.XXXXXX)"
OUTDIR="$TMPROOT/out"
mkdir -p "$OUTDIR"
LOG="$TMPROOT/selftest.log"

cleanup() {
  rm -rf "$TMPROOT"
}
trap cleanup EXIT

pass() { echo "PASS: $1"; }
fail() { echo "FAIL: $1" >&2; echo "--- log ---" >&2; tail -120 "$LOG" >&2 || true; exit 1; }

cd "$REPO_DIR"

FIXTURE="tests/fixtures/ui_shot/dm.json"
SHOT_A="$OUTDIR/dm-a.png"
SHOT_B="$OUTDIR/dm-b.png"
MONTAGE="$OUTDIR/dm-compare.png"

echo "selftest: temp=$TMPROOT"

# 1) Same fixture, same size -> comparable pair with equal dims/checksum.
UI_SHOT_CARGO_LOCK="${UI_SHOT_CARGO_LOCK:-/tmp/discord-rs-cargo-B.lock}" \
  bash scripts/ui-shot.sh dm "$SHOT_A" 640x360 >"$LOG" 2>&1 || fail "ui-shot first capture failed"
UI_SHOT_CARGO_LOCK="${UI_SHOT_CARGO_LOCK:-/tmp/discord-rs-cargo-B.lock}" \
  bash scripts/ui-shot.sh dm "$SHOT_B" 640x360 >>"$LOG" 2>&1 || fail "ui-shot second capture failed"

[[ -f "$SHOT_A" && -f "$SHOT_B" ]] || fail "expected captures missing"
SA="$(identify -format '%wx%h' "$SHOT_A")"
SB="$(identify -format '%wx%h' "$SHOT_B")"
[[ "$SA" == "640x360" && "$SB" == "640x360" ]] || fail "unexpected dimensions: $SA $SB"
HA="$(sha256sum "$SHOT_A" | awk '{print $1}')"
HB="$(sha256sum "$SHOT_B" | awk '{print $1}')"
[[ "$HA" == "$HB" ]] || fail "same fixture produced different checksums: $HA $HB"
pass "same fixture yields same dimensions and checksum"

bash scripts/ui-compare.sh "$SHOT_A" "$SHOT_B" "$MONTAGE" >>"$LOG" 2>&1 || fail "ui-compare failed on comparable pair"
[[ -f "$MONTAGE" ]] || fail "montage not created"
[[ -f "$OUTDIR/dm-compare.meta.json" ]] || fail "compare metadata missing"
grep -q '"ae_pixels": 0' "$OUTDIR/dm-compare.meta.json" || fail "expected AE 0 for identical pair"
pass "ui-compare produces comparable pair metadata with AE 0"

# 2) Black image must fail explicitly.
python3 - "$OUTDIR/black.png" <<'PY'
import struct,zlib,sys
w,h=64,64
def chunk(t,d):
    c=struct.pack('>I',len(d))+t+d
    return c+struct.pack('>I',zlib.crc32(c[4:])&0xffffffff)
raw=b''.join(b'\x00'+b'\x00\x00\x00'*w for _ in range(h))
png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',w,h,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(raw))+chunk(b'IEND',b'')
open(sys.argv[1],'wb').write(png)
PY
if bash scripts/ui-compare.sh "$OUTDIR/black.png" "$SHOT_A" "$OUTDIR/black-compare.png" >>"$LOG" 2>&1; then
  fail "ui-compare should fail on black image"
fi
tail -40 "$LOG" | grep -qi "preta/vazia" || fail "black-image failure message not explicit"
pass "ui-compare fails explicitly on black/blank image"

# 3) Timeout must fail explicitly. Use tiny capture timeout and force fallback off by PATH shim.
FAKEBIN="$TMPROOT/fakebin"
mkdir -p "$FAKEBIN"
cat > "$FAKEBIN/import" <<'FAKE'
#!/usr/bin/env bash
sleep 30
FAKE
cat > "$FAKEBIN/xwd" <<'FAKE'
#!/usr/bin/env bash
sleep 30
FAKE
chmod +x "$FAKEBIN/import" "$FAKEBIN/xwd"
# symlink required tools
for c in timeout bash env python3 sha256sum awk sed grep cat mkdir rm mv cp mktemp flock cargo setsid Xvfb xdpyinfo xdotool identify compare magick convert date head tail wc tr readlink seq sleep kill; do
  p="$(command -v "$c" 2>/dev/null || true)"
  [[ -n "$p" ]] && ln -s "$p" "$FAKEBIN/$c"
done

if PATH="$FAKEBIN:$PATH" UI_SHOT_CAPTURE_TIMEOUT_SECS=1 UI_SHOT_READY_TIMEOUT_SECS=3 \
  bash scripts/ui-shot.sh dm "$OUTDIR/timeout.png" 320x180 >>"$LOG" 2>&1; then
  fail "ui-shot should fail on capture timeout/black"
fi
tail -60 "$LOG" | grep -Eqi "timeout|captura|preta/vazia|nenhum frame" || fail "timeout failure message not explicit"
pass "ui-shot fails explicitly on capture timeout"

# 4) Metadata must not contain personal-data keys.
for meta in "$SHOT_A.meta.json" "$SHOT_B.meta.json" "$OUTDIR/dm-compare.meta.json"; do
  [[ -f "$meta" ]] || fail "missing metadata: $meta"
  python3 - "$meta" <<'PY'
import json,sys,re
obj=json.load(open(sys.argv[1]))
text=json.dumps(obj).lower()
forbidden=['username','discriminator','email','token','message','content','avatar','guild_id','channel_id','recipient','user_id']
bad=[k for k in forbidden if re.search(r'"'+re.escape(k)+r'"', text)]
if bad:
    raise SystemExit('forbidden keys: '+','.join(bad))
PY
done
pass "metadata sidecars contain no personal-data keys"

echo "SELFTEST_OK"
