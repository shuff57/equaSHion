// Buy Me a Coffee: a third button in the report buttons' column, the in-page invitation, and the
// height cap on BMC's panel. Ported from inishial (public/bmc.js and its rules in app.css), with
// one change of shape: inishial pins BMC's own chip top right; here that chip is hidden and the
// mug button in the bottom-right column drives it, so the corner has one stack of buttons and
// nothing sits over the equation editor.
//
// BMC's hosted widget (the <script data-name="BMC-Widget"> tag in index.html) loads from
// cdnjs.buymeacoffee.com and injects the chip (#bmc-wbtn) and the payment panel
// (#bmc-iframe). No backend, no keys. The page only has to click the chip.
(function () {
  'use strict';
  if (window.equashionCoffee) return;
  // BMC's loader has to be on the page for any of this to mean anything.
  if (!document.querySelector('script[data-name="BMC-Widget"]')) return;

  var INVITE_KEY = 'equashion.coffee-invited';
  var MUG =
    '<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
    '<path d="M4.3 8.6c3.7-.2 7.4-.3 11.1-.1.2 3.3.2 6.6 0 9.8-3.5.3-7.1.3-10.6.1-1-.3-1.6-1.1-1.7-2.1-.1-2.6-.1-5.1.1-7.7Z"/>' +
    '<path opacity=".55" d="M4.7 9.1c3.3-.2 6.7-.2 10.1-.1.1 2.9.1 5.8-.1 8.7-3.1.2-6.4.2-9.6.1-.7-.3-1.1-1-1.2-1.7-.1-2.4-.1-4.7.1-7Z"/>' +
    '<path d="M15.4 10.5c2.5-.2 4.7 1 5 3 .3 1.7-.9 3-2.5 3.3-1.2.2-2.4 0-3.5-.4"/>' +
    '<path d="M8.4 5.6c-.3-1.5-.3-2.9 0-4.4M11.7 5.4c-.1-1.4-.1-2.7 0-4"/></svg>';

  var CSS = [
    // BMC's own chip is driven from the mug button, so it stays out of sight and out of the way.
    '#bmc-wbtn { opacity: 0 !important; pointer-events: none !important; }',
    // BMC injects a thank-you message div with no class or id, position: fixed, as a child of
    // body. The empty message would draw as a stray box; its overlay wrapper (top: 0; left: 0)
    // is the one div that must stay.
    'body > div[style*="position: fixed"]:not([id]):not([class]):not([style*="top: 0"]):not([style*="top:0"]) { display: none !important; }',
    // The panel opens beside the button column, growing up from the bottom right. Its height is
    // capped here, not in script: BMC ships !important rules and min-height: 680px, and
    // min-height always beats max-height, so the min has to be released first.
    'iframe#bmc-iframe, iframe[src*="buymeacoffee.com"] { min-height: 0 !important; max-height: calc(100vh - 48px) !important; max-height: calc(100dvh - 48px) !important; transform-origin: right bottom !important; }',
    '@media (min-width: 640px) { iframe#bmc-iframe, iframe[src*="buymeacoffee.com"] { bottom: 24px !important; right: 84px !important; } }',
    // The invitation: the sentence BMC's chip is too small to carry, in the report panel's style.
    '.bmc-invite { position: fixed; right: 84px; bottom: 136px; z-index: 1098; width: min(260px, calc(100vw - 112px)); padding: 12px 14px; background: var(--card-bg, #fcfcfc); color: var(--fg, #2a2a30); border: 1px solid var(--border, #dcdce0); border-radius: 12px; box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25); font: 13px/1.4 system-ui, -apple-system, "Segoe UI", sans-serif; cursor: pointer; }',
    '.bmc-invite[hidden] { display: none; }',
    '.bmc-invite p { margin: 0; }',
    '@media (min-width: 640px) and (max-width: 1024px) { .bmc-invite { right: 76px; bottom: 128px; } }',
    '@media (max-width: 639px) { .bmc-invite { display: none; } }',
    '@media print { .bmc-invite { display: none !important; } }',
  ].join('\n');

  var style = document.createElement('style');
  style.id = 'equashion-coffee-styles';
  style.textContent = CSS;
  document.head.appendChild(style);

  var chipClicks = 0;
  // BMC's loader is async: the chip may not exist yet on a fast click. Poll briefly, then drive
  // it. 50 tries x 100 ms is a 5 s ceiling; BMC injects the chip within a second on a typical
  // page. Without the network the poll gives up and nothing happens.
  function openCoffee() {
    if (window.equashionReportUi) window.equashionReportUi.close();
    dismiss();
    var n = 0;
    (function openWhenReady() {
      var chip = document.getElementById('bmc-wbtn');
      if (chip) {
        chip.click();
        chipClicks += 1;
        capSoon();
      } else if (++n < 50) setTimeout(openWhenReady, 100);
    })();
  }

  // The invitation. Shown once per browser session, a few seconds after the page loads, and gone
  // after ten seconds, on a click, or when the report buttons are used.
  var invite = document.createElement('aside');
  invite.className = 'bmc-invite';
  invite.hidden = true;
  invite.setAttribute('aria-label', 'Buy the author a coffee');
  invite.innerHTML = '<p>If equaSHion saved you time, please consider buying me a coffee. Thank you.</p>';
  document.body.appendChild(invite);
  var dismissed = false;
  function dismiss() {
    if (dismissed) return;
    dismissed = true;
    invite.hidden = true;
  }
  invite.addEventListener('click', openCoffee);
  document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape') dismiss();
  });

  function showInviteOnce() {
    try {
      if (sessionStorage.getItem(INVITE_KEY)) return;
      sessionStorage.setItem(INVITE_KEY, '1');
    } catch (_) {
      return;
    }
    setTimeout(function () {
      if (dismissed) return;
      invite.hidden = false;
      setTimeout(dismiss, 10000);
    }, 4000);
  }

  // BMC re-sets iframe.style.height from the window height on every open, so the cap is
  // re-applied for a few frames after the click (bounded: an unbounded requestAnimationFrame
  // loop would keep a phone awake for the rest of the session).
  function capIframe() {
    var f = document.getElementById('bmc-iframe');
    if (!f) return;
    var cap = Math.min(560, Math.floor(window.innerHeight * 0.7));
    if (parseInt(f.style.height, 10) !== cap) f.style.height = cap + 'px';
  }
  function capSoon() {
    var n = 0;
    (function tick() {
      capIframe();
      if (++n < 10) requestAnimationFrame(tick);
    })();
  }

  // The button joins the report buttons' column once report.js has built it.
  function addButton() {
    var ui = window.equashionReportUi;
    if (!ui || !ui.addTool) return setTimeout(addButton, 100);
    ui.addTool('coffee', 'Buy me a coffee', MUG, openCoffee);
    showInviteOnce();
  }
  addButton();

  // Opening the report panel puts the invitation away.
  document.addEventListener('click', function (e) {
    if (e.target.closest && e.target.closest('.fab-toolbar') && !e.target.closest('[data-tool-id=coffee]')) dismiss();
  });

  window.equashionCoffee = {
    open: openCoffee,
    clicks: function () {
      return chipClicks;
    },
  };
})();
