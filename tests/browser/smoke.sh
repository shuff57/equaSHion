#!/usr/bin/env bash
# Browser smoke test of the web page. Run through `cargo xtask smoke` (which serves dist/),
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
# Wait for egui to paint a few frames.
frames() { pw --raw eval "async () => { for (let i = 0; i < ${1:-8}; i++) await new Promise(r => requestAnimationFrame(r)); return true; }" >/dev/null; }

ROWS="$(grep -v '^#' "$ROOT/corpus/equations.tsv" | grep -c .)"
echo "smoke: $URL (corpus rows: $ROWS)"

pw open "$URL" >/dev/null
pw resize 1280 900 >/dev/null

# The hooks appear once eframe has started the app (WebGL2 context, first frame).
r="$(pw --raw eval "async () => { for (let i = 0; i < 300; i++) { if (window.equashion && window.equashion.ready) return true; await new Promise(r => setTimeout(r, 100)); } return document.body.innerText; }")"
[ "$r" = "true" ] || fail "hooks never appeared: $r"
echo "smoke: ok   hooks installed"

# One view: activeTab is always editor, setTab accepts and ignores.
check "activeTab is editor" "eq.activeTab() === 'editor' || eq.activeTab()"
check "setTab accepts and ignores" "eq.setTab('gallery') === true && eq.setTab('nope') === true && eq.activeTab() === 'editor' || eq.activeTab()"
check "setLinear(\\\\frac{a}{b}) round-trips" "(() => { const l1 = eq.setLinear('\\\\frac{a}{b}'); if (!eq.omml().includes('<m:f>')) return 'no fraction: ' + eq.omml(); const l2 = eq.setLinear(l1); return (l2 === l1 && eq.linear() === l1) || ('linear ' + l1 + ' -> ' + l2); })()"
check "setLatex round-trips" "(() => { const a = eq.setLatex('\\\\frac{a}{b}+\\\\sqrt{x}'); const b = eq.setLatex(a); const c = eq.setLatex('\$\$x^2\$\$'); return (a === '\\\\frac{a}{b}+\\\\sqrt{x}' && b === a && eq.latex() === c && c === 'x^2') || [a, b, c]; })()"
check "selftest().ok" "(() => { const s = eq.selftest(); return s.ok === true || s; })()"
check "galleryCount() === $ROWS" "eq.galleryCount() === $ROWS || eq.galleryCount()"
check "unknownCommands()" "(() => { eq.setLinear('\\\\foo+x'); const u = eq.unknownCommands(); eq.setLinear('\\\\frac{a}{b}'); return (Array.isArray(u) && u.some(c => c.includes('foo'))) || u; })()"

# Copy payloads (the hook builds them and tries the real clipboard; without a click the
# browser may refuse, so the status is copied or blocked).
check "copy('latex')" "eq.setLatex('\\\\frac{a}{b}') && eq.copy('latex').then(r => (r.kind === 'latex' && ['copied', 'blocked'].includes(r.status) && r.items['text/plain'] === '\\\\frac{a}{b}') || r)"
# MathML: the exact header, well-formed XML (the browser's own XML parser), the MathML namespace
# on the root, no named entities, and the same <math> element alone in the HTML fragment.
MATHML_OK="(t, h) => { const head = '<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"block\">'; const d = new DOMParser().parseFromString(t, 'application/xml'); const bad = d.getElementsByTagName('parsererror').length > 0; const root = d.documentElement; const ok = t.startsWith(head) && !bad && root.localName === 'math' && root.namespaceURI === 'http://www.w3.org/1998/Math/MathML' && !/&[A-Za-z]/.test(t) && !t.includes('mfenced') && h === '<html><body><!--StartFragment-->' + t + '<!--EndFragment--></body></html>'; return ok || [t.slice(0, 120), bad, root && root.namespaceURI, h.slice(0, 120)]; }"
check "copy('mathml') is well-formed MathML" "eq.setLatex('\\\\frac{a}{b}') && eq.copy('mathml').then(r => r.kind === 'mathml' && Object.keys(r.items).join() === 'text/plain,text/html' ? ($MATHML_OK)(r.items['text/plain'], r.items['text/html']) : r)"
check "  with a fraction" "eq.copy('mathml').then(r => r.items['text/plain'].includes('<mfrac><mi>a</mi><mi>b</mi></mfrac>') || r.items['text/plain'])"
check "  for the quadratic formula" "eq.loadExample('quadratic') && eq.copy('mathml').then(r => { const t = r.items['text/plain']; const e = ($MATHML_OK)(t, r.items['text/html']); return e === true ? (t.includes('<msqrt>') && t.includes('<msup>')) || t : e; })"
check "copy('word') is the mathml alias" "eq.copy('word').then(r => r.kind === 'mathml' || r.kind)"
check "copy('omml') is OMML with m:f" "eq.setLatex('\\\\frac{a}{b}') && eq.copy('omml').then(r => { const t = r.items['text/plain']; return (r.kind === 'omml' && Object.keys(r.items).join() === 'text/plain' && t.startsWith('<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\"') && t.includes('<m:f>')) || [r.kind, t]; })"
check "copy('text') is linear" "eq.copy('text').then(r => r.items['text/plain'] === 'a/b' || r.items)"
check "copy('image') is a PNG" "eq.copy('image').then(r => { const b = r.items['image/png']; const magic = [137, 80, 78, 71, 13, 10, 26, 10]; return (b instanceof Uint8Array && b.length > 500 && magic.every((v, i) => b[i] === v)) || [typeof b, b && b.length]; })"
check "copy('svg') downloads an <svg>" "eq.copy('svg').then(r => (r.status === 'downloaded' && r.items['image/svg+xml'].startsWith('<svg')) || [r.status, r.items['image/svg+xml'].slice(0, 40)])"
# The confirmation on the button only lasts two seconds, so the status is read in the same
# evaluation as the copy that made it.
check "copy('svg') leaves copyStatus() downloaded" "eq.copy('svg').then(() => new Promise(r => setTimeout(r, 100))).then(() => { const s = eq.copyStatus(); return (s && s.kind === 'svg' && s.state === 'downloaded') || s; })"
check "copy rejects an unknown kind" "eq.copy('pdf').then(() => 'resolved', e => String(e).includes('unknown kind'))"

# A real click on Copy LaTeX: the copy event path runs inside the user gesture. Record what the
# page hands the clipboard (DataTransfer.setData) without needing clipboard-read permission.
pw --raw eval "() => { window.__eqSet = []; const o = DataTransfer.prototype.setData; DataTransfer.prototype.setData = function (t, d) { window.__eqSet.push([t, d]); return o.call(this, t, d); }; return true; }" >/dev/null
eq_rect() { pw --raw eval "() => { const r = window.equashion.copyButtonRect('$1'); return r ? Math.round(r.x + r.w / 2) + ' ' + Math.round(r.y + r.h / 2) : 'none'; }" | tr -d '"'; }
check "setLatex for the click" "eq.setLatex('x^2+1') === 'x^2+1'"
frames 4
read -r CX CY <<<"$(eq_rect latex)"
[ "$CX" != "none" ] || fail "no Copy LaTeX button rect"
pw mousemove "$CX" "$CY" >/dev/null; pw mousedown >/dev/null; pw mouseup >/dev/null
frames 8
check "click Copy LaTeX writes text/plain" "(async () => { const s = await (async () => { for (let i = 0; i < 60; i++) { const x = eq.copyStatus(); if (x && x.state !== 'pending') return x; await new Promise(r => setTimeout(r, 50)); } return eq.copyStatus() || {}; })(); const w = window.__eqSet; return (s.kind === 'latex' && s.state === 'copied' && w.some(([t, d]) => t === 'text/plain' && d === 'x^2+1')) || [s, w]; })()"
check "setLatex for the MathML click" "eq.setLatex('\\\\frac{1}{2}') === '\\\\frac{1}{2}'"
check "open the format menu" "eq.showMoreCopy(true) === true"
frames 4
read -r CX CY <<<"$(eq_rect mathml)"
[ "$CX" != "none" ] || fail "no MathML menu item rect"
pw mousemove "$CX" "$CY" >/dev/null; pw mousedown >/dev/null; pw mouseup >/dev/null
frames 4
read -r CX CY <<<"$(eq_rect mathml)"
[ "$CX" != "none" ] || fail "no Copy MathML button rect"
pw --raw eval "() => { window.__eqSet = []; return true; }" >/dev/null
pw mousemove "$CX" "$CY" >/dev/null; pw mousedown >/dev/null; pw mouseup >/dev/null
frames 8
check "click Copy MathML writes MathML as text and HTML" "(async () => { const s = await (async () => { for (let i = 0; i < 60; i++) { const x = eq.copyStatus(); if (x && x.state !== 'pending') return x; await new Promise(r => setTimeout(r, 50)); } return eq.copyStatus() || {}; })(); const w = window.__eqSet; const t = w.find(x => x[0] === 'text/plain'), h = w.find(x => x[0] === 'text/html'); return (s.kind === 'mathml' && s.state === 'copied' && t && t[1].startsWith('<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"block\">') && t[1].includes('<mfrac><mn>1</mn><mn>2</mn></mfrac>') && h && h[1].includes('<!--StartFragment-->' + t[1] + '<!--EndFragment-->')) || [s, w.map(x => x[0])]; })()"
# The second row: OMML, text and SVG are behind More.
check "OMML hidden until the menu opens" "eq.copyButtonRect('omml') === null || eq.copyButtonRect('omml')"
check "showMoreCopy(true)" "eq.showMoreCopy(true) === true"
frames 4
check "  every format is in the menu" "['latex', 'mathml', 'image', 'omml', 'text', 'svg'].every(k => eq.copyButtonRect(k) !== null) || 'missing'"
check "  menu items sit beside the Copy button, not over it" "(() => { const m = eq.copyButtonRect('mathml'), o = eq.copyButtonRect('omml'); return o.y > m.y + m.h || o.y + o.h < m.y || [m, o]; })()"
read -r CX CY <<<"$(eq_rect omml)"
pw mousemove "$CX" "$CY" >/dev/null; pw mousedown >/dev/null; pw mouseup >/dev/null
frames 4
read -r CX CY <<<"$(eq_rect omml)"
pw --raw eval "() => { window.__eqSet = []; return true; }" >/dev/null
pw mousemove "$CX" "$CY" >/dev/null; pw mousedown >/dev/null; pw mouseup >/dev/null
frames 8
check "click Copy OMML writes OMML text" "(async () => { const s = await (async () => { for (let i = 0; i < 60; i++) { const x = eq.copyStatus(); if (x && x.state !== 'pending') return x; await new Promise(r => setTimeout(r, 50)); } return eq.copyStatus() || {}; })(); const w = window.__eqSet; return (s.kind === 'omml' && s.state === 'copied' && w.length === 1 && w[0][0] === 'text/plain' && w[0][1].startsWith('<m:oMath xmlns:m=')) || [s, w]; })()"
check "open the menu for image" "eq.showMoreCopy(true) === true"
frames 4
read -r CX CY <<<"$(eq_rect image)"
pw mousemove "$CX" "$CY" >/dev/null; pw mousedown >/dev/null; pw mouseup >/dev/null
frames 4
read -r CX CY <<<"$(eq_rect image)"
pw mousemove "$CX" "$CY" >/dev/null; pw mousedown >/dev/null; pw mouseup >/dev/null
pw --raw eval "async () => { for (let i = 0; i < 40; i++) { const s = window.equashion.copyStatus(); if (s && s.kind === 'image' && s.state !== 'pending') return true; await new Promise(r => setTimeout(r, 50)); } return true; }" >/dev/null
pw mousemove 2 890 >/dev/null
IMG_STATE="$(pw --raw eval "() => JSON.stringify(window.equashion.copyStatus())")"
echo "smoke: info click Copy image -> $IMG_STATE (headless Chromium may refuse image/png without clipboard permission)"

# Paste into the editor: a real paste event, as Ctrl+V delivers it.
paste_js() {
    # $1: JS string expression of the pasted text
    echo "(() => { eq.setLatex(''); const dt = new DataTransfer(); dt.setData('text/plain', $1); document.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true })); return true; })()"
}
OMML="'<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\"><m:f><m:num><m:r><m:t>1</m:t></m:r></m:num><m:den><m:r><m:t>2</m:t></m:r></m:den></m:f></m:oMath>'"
check "paste event with OMML" "$(paste_js "$OMML")"
frames 4
check "  OMML loaded" "eq.latex() === '\\\\frac{1}{2}' || eq.latex()"
MML="'<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"block\"><msqrt><msup><mi>x</mi><mn>2</mn></msup><mo>+</mo><mn>1</mn></msqrt></math>'"
check "paste event with MathML" "$(paste_js "$MML")"
frames 4
check "  MathML loaded" "eq.latex() === '\\\\sqrt{x^2+1}' || eq.latex()"
check "paste hook reads MathML from copy('mathml')" "eq.setLatex('\\\\sum_{i=1}^n i^2') && eq.copy('mathml').then(r => { eq.setLatex(''); const l = eq.paste(r.items['text/plain']); return l === '\\\\sum_{i=1}^ni^2' || l; })"
check "paste hook refuses broken MathML" "(() => { eq.setLatex('y'); const r = eq.paste('<math><mrow>'); return (r === null && eq.latex() === 'y') || [r, eq.latex()]; })()"
check "paste event with LaTeX" "$(paste_js "'\$\\\\sqrt{x}+\\\\alpha\$'")"
frames 4
check "  LaTeX loaded" "eq.latex() === '\\\\sqrt{x}+\\\\alpha' || eq.latex()"
check "paste event with linear text" "$(paste_js "'a^2+b^2=c^2'")"
frames 4
check "  linear text loaded" "eq.latex() === 'a^2+b^2=c^2' || eq.latex()"
check "paste hook refuses broken OMML" "(() => { eq.setLatex('y'); const r = eq.paste('<m:oMath><m:f>'); return (r === null && eq.latex() === 'y') || [r, eq.latex()]; })()"
check "paste hook names unknown commands" "(() => { eq.setLatex(''); eq.paste('\\\\foo+1'); const u = eq.unknownCommands(); return u.some(c => c.includes('foo')) || u; })()"

# Examples.
check "examples load" "(() => { const ex = eq.examples(); if (ex.length < 8 || ex.length > 10) return ex; for (const e of ex) { const l = eq.loadExample(e.id); if (!l) return 'empty: ' + e.id; } return eq.loadExample('nope') === null || 'nope loaded'; })()"
check "load the quadratic formula" "eq.loadExample('quadratic').includes('\\\\frac{-b')"

check "theme hooks" "(() => { const p = eq.themePreference(); const t = eq.theme(); return (p === 'system' && (t === 'light' || t === 'dark')) || [p, t]; })()"
check "setTheme rejects an unknown name" "eq.setTheme('sepia') === false && eq.themePreference() === 'system' || eq.themePreference()"

# shot NAME THEME: save tests/browser/out/NAME.png after egui paints, and check it is not blank
# and has the theme's overall brightness.
shot() {
    local name="$1" theme="$2"
    check "setTheme($theme)" "eq.setTheme('$theme') && eq.theme() === '$theme' && eq.themePreference() === '$theme' || eq.theme()"
    frames 8
    pw screenshot --filename "$OUT/$name.png" >/dev/null
    python3 -I "$HERE/check_png.py" "$OUT/$name.png" "$theme" || fail "$name screenshot looks blank or has the wrong theme"
}
check "closed disclosures" "eq.setDisclosures(false, false) && eq.showAbout(false) && eq.showMoreCopy(false) === false"
for width in 1280 700; do
    pw resize "$width" 900 >/dev/null
    for theme in light dark; do
        shot "page-$theme-$width" "$theme"
    done
done
# Everything open, tall window: the whole page down to the About panel.
check "open disclosures" "eq.setDisclosures(true, true) && eq.setPaletteTab('Functions') && eq.showAbout(true) && eq.showMoreCopy(true)"
for width in 1280 700; do
    pw resize "$width" 2900 >/dev/null
    for theme in light dark; do
        shot "open-$theme-$width" "$theme"
    done
done
check "close disclosures" "eq.setPaletteTab('Structures') && eq.setDisclosures(false, false) && eq.showAbout(false) && eq.showMoreCopy(false) === false"
# The report buttons (report.js): two buttons bottom right, always visible, no menu. The accent
# one opens the "Report an issue" panel; an empty form is refused in the panel, a filled one is
# posted to this origin (xtask serve writes it under target/issue-reports) and the panel says so;
# the new report is then in the list (the small button) and a vote counts once. No sign-in.
pw resize 1280 900 >/dev/null
check "the report button is bottom right" "(() => { const b = document.querySelector('.fab-main-btn'); if (!b) return 'no button'; const r = b.getBoundingClientRect(); return (r.width === 48 && r.height === 48 && Math.abs(innerWidth - r.right - 24) < 1.5 && Math.abs(innerHeight - r.bottom - 24) < 1.5) || [r.x, r.y, r.width, r.height, innerWidth, innerHeight]; })()"
check "both buttons are showing and there is no menu" "(() => { const names = [...document.querySelectorAll('.fab-toolbar button')].map(b => b.getAttribute('aria-label')).join(); const vis = [...document.querySelectorAll('.fab-toolbar button')].every(b => { const r = b.getBoundingClientRect(); return r.width > 0 && getComputedStyle(b).opacity === '1'; }); return (names === 'Report a Bug,Reported issues' && vis) || [names, vis]; })()"
check "Report a Bug opens the panel with raSHio's layout" "(() => { document.querySelector('.fab-main-btn').click(); const p = document.getElementById('issue-orb-panel'); const kinds = [...p.querySelectorAll('.issue-report-kind-label')].map(x => x.textContent).join(); const labels = [...p.querySelectorAll('label')].map(x => x.textContent).join(); return (p.getAttribute('data-visible') === 'true' && p.querySelector('h4').textContent === 'Report an issue' && kinds === 'Bug,Quirk,Improve' && labels === 'Title,Description' && p.querySelector('.issue-report-submit').textContent === 'Send report') || [kinds, labels]; })()"
frames 8
pw screenshot --filename "$OUT/report.png" >/dev/null
check "an empty report is refused in the panel" "(async () => { const p = document.getElementById('issue-orb-panel'); p.querySelector('.issue-report-submit').click(); await new Promise(r => setTimeout(r, 100)); const e = p.querySelector('.issue-report-error'); return (!e.hidden && e.textContent === 'Add a short title.') || e.textContent; })()"
check "a report is sent from the panel" "(async () => { const p = document.getElementById('issue-orb-panel'); p.querySelector('[data-kind=enhancement]').click(); const t = p.querySelector('.issue-report-title'), d = p.querySelector('.issue-report-description'); t.value = 'Smoke test report'; d.value = 'Sent by tests/browser/smoke.sh.'; p.querySelector('.issue-report-submit').click(); for (let i = 0; i < 50; i++) { const th = p.querySelector('.issue-report-thanks'); if (!th.hidden) return th.textContent.startsWith('Thank you') || th.textContent; await new Promise(r => setTimeout(r, 100)); } return p.querySelector('.issue-report-error').textContent || 'no answer'; })()"
frames 4
check "the report is in the list and a vote counts once" "(async () => { await new Promise(r => setTimeout(r, 2600)); document.querySelector('[data-tool-id=issue-list]').click(); const p = document.getElementById('issue-orb-panel'); const items = () => [...p.querySelectorAll('.issue-item')].filter(i => i.querySelector('.issue-item-title').textContent === 'Smoke test report').sort((a, b) => Number(b.dataset.issueId) - Number(a.dataset.issueId)); for (let i = 0; i < 50 && !items().length; i++) await new Promise(r => setTimeout(r, 100)); if (!items().length) return 'not listed'; const count = () => Number(items()[0].querySelector('.issue-vote-count').textContent); const before = count(); const btn = items()[0].querySelector('.issue-vote'); btn.click(); for (let i = 0; i < 50 && btn.getAttribute('aria-pressed') !== 'true'; i++) await new Promise(r => setTimeout(r, 100)); btn.click(); await new Promise(r => setTimeout(r, 300)); return (count() === before + 1 && btn.disabled) || [before, count()]; })()"
frames 4
check "Escape closes the panel" "(() => { document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' })); return document.getElementById('issue-orb-panel').getAttribute('data-visible') === 'false' || 'still open'; })()"
pw resize 1280 900 >/dev/null
check "setTheme(system)" "eq.setTheme('system') && eq.themePreference() === 'system' || eq.themePreference()"
check "still on the editor with an equation" "eq.activeTab() === 'editor' && eq.latex().length > 0"
echo "smoke: passed (screenshots in $OUT)"
