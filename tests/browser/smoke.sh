#!/usr/bin/env bash
# Browser smoke test of the web showcase. Run through `cargo xtask smoke` (which serves dist/),
# or by hand: tests/browser/smoke.sh http://127.0.0.1:8780/
#
# Uses playwright-cli with its OWN session name (`equashion`), headless, and closes only that
# session at the end. It never uses close-all, never touches other sessions or browser profiles.
set -euo pipefail

URL="${1:-http://127.0.0.1:8780/}"
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="$HERE/out"
ROOT="$(cd "$HERE/../.." && pwd)"
SESSION=equashion
mkdir -p "$OUT"
# playwright-cli writes its snapshots into the working directory: keep them in out/.
cd "$OUT"

pw() { playwright-cli -s="$SESSION" "$@"; }
cleanup() { playwright-cli -s="$SESSION" close >/dev/null 2>&1 || true; }
trap cleanup EXIT

fail() { echo "smoke: FAIL: $*" >&2; exit 1; }
# check NAME JS: JS is an expression (may use `eq` = window.equashion) that yields true, or a
# string saying what is wrong.
check() {
    local name="$1" js="$2" r
    r="$(pw --raw eval "async () => { const eq = window.equashion; const r = await (async () => ($js))(); return r === true ? true : JSON.stringify(r); }")" || fail "$name: eval error: $r"
    [ "$r" = "true" ] || fail "$name: $r"
    echo "smoke: ok   $name"
}

ROWS="$(grep -v '^#' "$ROOT/corpus/equations.tsv" | grep -c .)"
echo "smoke: $URL (corpus rows: $ROWS)"

pw open "$URL" >/dev/null
pw resize 1280 900 >/dev/null

# The hooks appear once eframe has started the app (WebGL2 context, first frame).
r="$(pw --raw eval "async () => { for (let i = 0; i < 300; i++) { if (window.equashion && window.equashion.ready) return true; await new Promise(r => setTimeout(r, 100)); } return document.body.innerText; }")"
[ "$r" = "true" ] || fail "hooks never appeared: $r"
echo "smoke: ok   hooks installed"

check "activeTab is editor" "eq.activeTab() === 'editor' || eq.activeTab()"
check "setLinear(\\\\frac{a}{b}) round-trips" "(() => { const l1 = eq.setLinear('\\\\frac{a}{b}'); if (!eq.omml().includes('<m:f>')) return 'no fraction: ' + eq.omml(); const l2 = eq.setLinear(l1); return (l2 === l1 && eq.linear() === l1) || ('linear ' + l1 + ' -> ' + l2); })()"
check "selftest().ok" "(() => { const s = eq.selftest(); return s.ok === true || s; })()"
check "galleryCount() === $ROWS" "eq.galleryCount() === $ROWS || eq.galleryCount()"
check "unknownCommands()" "(() => { eq.setLinear('\\\\foo+x'); const u = eq.unknownCommands(); eq.setLinear('\\\\frac{a}{b}'); return (Array.isArray(u) && u.some(c => c.includes('foo'))) || u; })()"

shot() {
    local tab="$1"
    check "setTab($tab)" "eq.setTab('$tab') && eq.activeTab() === '$tab'"
    # Two animation frames so egui paints the new tab before the capture.
    pw --raw eval "async () => { for (let i = 0; i < 6; i++) await new Promise(r => requestAnimationFrame(r)); return true; }" >/dev/null
    pw screenshot --filename "$OUT/$tab.png" >/dev/null
    python3 -I "$HERE/check_png.py" "$OUT/$tab.png" || fail "$tab screenshot looks blank"
}
shot selftest
shot gallery
shot omml
shot editor
check "back on editor with a fraction" "eq.activeTab() === 'editor' && eq.linear().length > 0"
echo "smoke: passed (screenshots in $OUT)"
