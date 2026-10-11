// The Report a Bug button and panel: a page element over the canvas, laid out and styled like
// raSHio's (src/fabToolbar.js, src/issueReport.js): two buttons bottom right, always visible,
// no menu. The accent one opens the "Report an issue" panel (three kinds, title, description,
// optional screenshot, Send report). The small one above it opens the public list of what was
// reported, with a Vote button on each.
//
// No sign-in. The report goes to POST /api/issue-reports on this origin (see
// docs/issue-reports.md): the local server writes it to target/issue-reports, the deployed
// functions file it as a GitHub issue. The page's colours come from CSS variables that follow
// <html data-theme="light|dark">, which src/web.rs keeps in step with the egui theme.
(function () {
  'use strict';
  if (window.equashionReportUi) return;

  // The kinds, with the same labels, hints and colours as raSHio's issueKinds.js.
  var KINDS = [
    { value: 'bug', label: 'Bug', hint: "doesn't work", color: '#f87171', ink: '#17171a' },
    { value: 'quirk', label: 'Quirk', hint: 'works, but picky', color: '#fbbf24', ink: '#17171a' },
    { value: 'enhancement', label: 'Improve', hint: 'wish it had', color: '#22c55e', ink: '#17171a' },
  ];
  var PLACEHOLDERS = {
    bug: { title: 'Matrix cells lose the caret after Tab', body: 'What did you try, and what happened instead?' },
    quirk: { title: 'Typing sqrt needs a space before the argument', body: 'What feels off or picky about how this works?' },
    enhancement: { title: 'Let me export the equation as a Word file', body: 'What would you like to be able to do?' },
  };
  var STATUS_LABELS = { open: 'Open', in_progress: 'In progress', fixed: 'Fixed', deferred: 'Deferred' };
  var KIND_LABELS = { bug: 'Bug', quirk: 'Quirk', enhancement: 'Improve' };
  var MAX_SHOT_CHARS = 1400000;
  var MAX_SHOT_TEXT = '1 MB';
  var VOTED_KEY = 'equashion.voted';

  var BUG_ICON =
    '<svg width="20" height="20" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
    '<path d="M5.5 5.5a2.5 2.5 0 0 1 5 0"/><rect x="4.5" y="5.5" width="7" height="7.5" rx="3.5"/><path d="M8 5.5V13M4.5 8H1.5M11.5 8h3M4.8 11l-2.3 1.7M11.2 11l2.3 1.7M5.2 3.4 3.6 1.8M10.8 3.4l1.6-1.6"/></svg>';
  var LIST_ICON =
    '<svg width="20" height="20" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" aria-hidden="true">' +
    '<path d="M5.5 4h8M5.5 8h8M5.5 12h8"/><circle cx="2.5" cy="4" r=".6" fill="currentColor"/><circle cx="2.5" cy="8" r=".6" fill="currentColor"/><circle cx="2.5" cy="12" r=".6" fill="currentColor"/></svg>';
  var CSS = [
    // Theme tokens: the page's own palette (src/theme.rs), under raSHio's variable names.
    ':root { --card-bg: #fcfcfc; --fg: #2a2a30; --muted: #5c5c66; --border: #dcdce0; --input-bg: #efeff1; --accent: #2b62a3; --accent-fill: #2b62a3; --on-accent: #fcfcfc; --shadow: rgba(0, 0, 0, 0.25); --err: #b42318; }',
    ':root[data-theme="dark"] { --card-bg: #1b1b1e; --fg: #dcdce1; --muted: #a3a3ad; --border: #2c2c31; --input-bg: #0f0f11; --accent: #7aaee6; --accent-fill: #7aaee6; --on-accent: #141416; --shadow: rgba(0, 0, 0, 0.5); --err: #f2877d; }',
    '@media (prefers-color-scheme: dark) { :root:not([data-theme]) { --card-bg: #1b1b1e; --fg: #dcdce1; --muted: #a3a3ad; --border: #2c2c31; --input-bg: #0f0f11; --accent: #7aaee6; --accent-fill: #7aaee6; --on-accent: #141416; --shadow: rgba(0, 0, 0, 0.5); --err: #f2877d; } }',

    // The button (raSHio fabToolbar.js).
    '.fab-toolbar { position: fixed; bottom: 24px; right: 24px; z-index: 1100; display: flex; flex-direction: column-reverse; align-items: center; gap: 0; pointer-events: none; font: 13px/1.4 system-ui, -apple-system, "Segoe UI", sans-serif; }',
    '.fab-main-btn { width: 48px; height: 48px; border: none; border-radius: 14px; background: var(--accent-fill); color: var(--on-accent); cursor: pointer; display: flex; align-items: center; justify-content: center; transition: filter 0.2s ease; box-shadow: 0 2px 8px var(--shadow); pointer-events: auto; position: relative; z-index: 1; padding: 0; }',
    '.fab-main-btn:hover { filter: brightness(1.1); }',
    '.fab-main-btn:focus-visible, .fab-sub-btn:focus-visible { outline: 2px solid var(--fg); outline-offset: 2px; }',
    '.fab-main-btn svg { width: 24px; height: 24px; pointer-events: none; }',
    '.fab-main-btn .fab-tooltip { position: absolute; right: calc(100% + 8px); top: 50%; transform: translateY(-50%); background: var(--card-bg); color: var(--fg); font-size: 12px; font-weight: 500; padding: 4px 8px; border-radius: 4px; white-space: nowrap; box-shadow: 0 2px 6px var(--shadow); pointer-events: none; opacity: 0; transition: opacity 0.15s ease; }',
    '.fab-main-btn:hover .fab-tooltip, .fab-main-btn:focus-visible .fab-tooltip { opacity: 1; }',
    '.fab-sub-container { display: flex; flex-direction: column-reverse; align-items: center; gap: 12px; padding-bottom: 12px; pointer-events: none; }',
    '.fab-sub-btn { width: 40px; height: 40px; border: none; border-radius: 12px; background: var(--card-bg); color: var(--fg); cursor: pointer; font-size: 20px; display: flex; align-items: center; justify-content: center; box-shadow: 0 2px 8px var(--shadow); pointer-events: auto; transition: filter 0.2s ease; position: relative; padding: 0; }',
    '.fab-sub-btn:hover { filter: brightness(1.15); }',
    '.fab-sub-btn .fab-tooltip { position: absolute; right: calc(100% + 8px); top: 50%; transform: translateY(-50%); background: var(--card-bg); color: var(--fg); font-size: 12px; font-weight: 500; padding: 4px 8px; border-radius: 4px; white-space: nowrap; box-shadow: 0 2px 6px var(--shadow); pointer-events: none; opacity: 0; transition: opacity 0.15s ease; }',
    '.fab-sub-btn:hover .fab-tooltip, .fab-sub-btn:focus-visible .fab-tooltip { opacity: 1; }',
    '@media (min-width: 640px) and (max-width: 1024px) { .fab-toolbar { bottom: 16px; right: 16px; } .issue-orb-panel { bottom: 128px; right: 16px; } }',
    '@media print { .fab-toolbar, .issue-orb-panel { display: none !important; } }',
    '@media (prefers-reduced-motion: reduce) { .fab-main-btn, .fab-main-btn .fab-tooltip, .fab-sub-btn, .fab-sub-btn .fab-tooltip, .issue-orb-panel { transition: none !important; } }',

    // The panel (raSHio issueReport.js).
    '.issue-orb-panel { position: fixed; bottom: 136px; right: 24px; width: min(340px, calc(100vw - 32px)); max-height: calc(100vh - 160px); overflow-y: auto; background: var(--card-bg); color: var(--fg); border: 1px solid var(--border); border-radius: 12px; box-shadow: 0 16px 40px rgba(0, 0, 0, 0.35); padding: 18px 20px; z-index: 1099; opacity: 0; transform: translateY(12px); pointer-events: none; visibility: hidden; transition: opacity 0.2s ease, transform 0.2s ease, visibility 0s linear 0.2s; font: 13px/1.4 system-ui, -apple-system, "Segoe UI", sans-serif; box-sizing: border-box; }',
    '.issue-orb-panel[data-visible="true"] { opacity: 1; transform: translateY(0); pointer-events: auto; visibility: visible; transition-delay: 0s; }',
    '.issue-orb-panel h4 { margin: 0 0 12px 0; font-size: 16px; font-weight: 600; }',
    '.issue-orb-close { position: absolute; top: 8px; right: 10px; background: none; border: none; color: var(--muted); font-size: 18px; cursor: pointer; padding: 4px; line-height: 1; }',
    '.issue-orb-close:hover { color: var(--fg); }',
    '.issue-orb-panel label { display: block; font-size: 12px; font-weight: 600; color: var(--muted); margin-bottom: 4px; }',
    '.issue-orb-panel input[type="text"], .issue-orb-panel textarea { display: block; width: 100%; padding: 7px 10px; font-size: 13px; color: var(--fg); background: var(--input-bg); border: 1px solid var(--border); border-radius: 6px; margin-bottom: 10px; box-sizing: border-box; font-family: inherit; }',
    '.issue-orb-panel input[type="text"]:focus, .issue-orb-panel textarea:focus { outline: none; border-color: var(--accent); }',
    '.issue-orb-panel textarea { resize: vertical; min-height: 60px; }',
    '.issue-report-kinds { display: flex; gap: 6px; margin-bottom: 12px; }',
    '.issue-report-kind { flex: 1; padding: 6px 4px; border: 1px solid var(--border); border-radius: 6px; background: transparent; color: var(--fg); font-family: inherit; cursor: pointer; text-align: center; }',
    '.issue-report-kind[aria-pressed="true"] { border-width: 2px; padding: 5px 3px; }',
    '.issue-report-kind-label { display: block; font-size: 14px; font-weight: 700; }',
    '.issue-report-kind-hint { display: block; font-size: 11px; opacity: 0.75; }',
    '.issue-report-file { display: none !important; }',
    '.issue-report-shot { display: flex; align-items: center; gap: 8px; margin-bottom: 10px; }',
    '.issue-report-shot-status { pointer-events: none; flex: 0 1 auto; font-size: 11px; color: var(--muted); opacity: 0.7; }',
    '.issue-report-thumb:not([hidden]) { height: 36px; max-width: 72px; object-fit: cover; border: 1px solid var(--border); border-radius: 4px; display: block; }',
    '.issue-report-attach, .issue-report-remove { flex: 0 0 auto; background: transparent; border: 1px solid var(--border); color: var(--fg); padding: 5px 10px; border-radius: 4px; font-size: 12px; font-family: inherit; cursor: pointer; }',
    '.issue-report-attach:disabled, .issue-report-remove:disabled { cursor: wait; opacity: 0.6; }',
    '.issue-report-error { margin: 0 0 10px; padding: 7px 9px; font-size: 12px; color: var(--err); background: rgba(220, 38, 38, 0.12); border: 1px solid var(--err); border-radius: 4px; }',
    '.issue-report-note { margin: 0 0 12px; font-size: 11px; color: var(--muted); opacity: 0.7; }',
    '.issue-report-submit { display: block; width: 100%; padding: 8px 14px; font-size: 13px; font-weight: 600; color: #fff; background: var(--accent); border: none; border-radius: 6px; cursor: pointer; transition: filter 0.15s ease; margin-top: 4px; font-family: inherit; }',
    '.issue-report-submit:hover { filter: brightness(1.1); }',
    '.issue-report-submit:disabled { opacity: 0.6; cursor: not-allowed; }',
    '.issue-report-thanks { margin: 4px 0 4px; font-size: 13px; }',
    '.issue-orb-link { background: none; border: none; padding: 0; margin: 10px 0 0; color: var(--accent); font: inherit; font-size: 12px; cursor: pointer; text-decoration: underline; }',

    // The list of what was reported.
    '.issue-list-empty { margin: 0 0 8px; font-size: 12px; color: var(--muted); }',
    '.issue-item { display: flex; gap: 10px; padding: 10px 0; border-top: 1px solid var(--border); }',
    '.issue-item:first-of-type { border-top: none; padding-top: 2px; }',
    '.issue-vote { flex: 0 0 auto; align-self: flex-start; min-width: 52px; padding: 5px 8px; border: 1px solid var(--border); border-radius: 6px; background: transparent; color: var(--fg); font: inherit; font-size: 12px; font-weight: 600; cursor: pointer; text-align: center; }',
    '.issue-vote:hover:not(:disabled) { border-color: var(--accent); color: var(--accent); }',
    '.issue-vote[aria-pressed="true"] { border-color: var(--accent); background: var(--accent); color: var(--on-accent); cursor: default; }',
    '.issue-vote:disabled { opacity: 0.7; }',
    '.issue-vote-count { display: block; font-size: 14px; font-weight: 700; }',
    '.issue-item-body { min-width: 0; flex: 1; }',
    '.issue-item-title { font-size: 13px; font-weight: 600; overflow-wrap: anywhere; }',
    '.issue-item-meta { font-size: 11px; color: var(--muted); margin: 1px 0 3px; }',
    '.issue-item-meta a { color: var(--accent); }',
    '.issue-item-desc { font-size: 12px; color: var(--fg); opacity: 0.85; overflow-wrap: anywhere; }',
  ].join('\n');

  var main = null;
  var subs = null;
  var panel = null;
  var view = 'form';
  var selectedKind = KINDS[0].value;
  var shot = null; // { name, dataUrl }
  var busy = false;
  var listLoaded = false;
  var voting = {};

  function el(tag, cls, html) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (html !== undefined) e.innerHTML = html;
    return e;
  }

  function text(s) {
    return document.createTextNode(String(s));
  }

  function voted() {
    try {
      return JSON.parse(localStorage.getItem(VOTED_KEY) || '[]');
    } catch (_) {
      return [];
    }
  }

  function remember(id) {
    var v = voted();
    if (v.indexOf(String(id)) < 0) v.push(String(id));
    try {
      localStorage.setItem(VOTED_KEY, JSON.stringify(v));
    } catch (_) {}
  }

  function api() {
    return window.equashion || {};
  }

  // ---- the button ---------------------------------------------------------------------------

  function addTool(id, label, icon, onClick) {
    var b = el('button', 'fab-sub-btn');
    b.type = 'button';
    b.dataset.toolId = id;
    b.setAttribute('aria-label', label);
    b.innerHTML = icon;
    var tip = el('span', 'fab-tooltip');
    tip.textContent = label;
    b.appendChild(tip);
    b.addEventListener('click', function (e) {
      e.stopPropagation();
      onClick();
    });
    subs.appendChild(b);
  }

  // ---- the panel ----------------------------------------------------------------------------

  function setVisible(on) {
    if (!on) {
      showError('');
    }
    panel.setAttribute('data-visible', on ? 'true' : 'false');
    if (on) {
      var first = panel.querySelector(view === 'form' ? '.issue-report-title' : '.issue-orb-close');
      if (first && first.focus) first.focus({ preventScroll: true });
    }
  }

  function isVisible() {
    return panel.getAttribute('data-visible') === 'true';
  }

  function showView(name) {
    view = name;
    panel.querySelector('.issue-view-form').hidden = name !== 'form';
    panel.querySelector('.issue-view-list').hidden = name !== 'list';
    if (name === 'list') loadList();
  }

  function openPanel(name) {
    if (isVisible() && view === name) {
      setVisible(false);
      return;
    }
    showView(name);
    setVisible(true);
  }

  var errorBox = null;
  function showError(message) {
    if (!errorBox) return;
    errorBox.textContent = message || '';
    errorBox.hidden = !message;
  }

  function build() {
    var style = document.createElement('style');
    style.id = 'equashion-report-styles';
    style.textContent = CSS;
    document.head.appendChild(style);

    // The button.
    var bar = el('div', 'fab-toolbar');
    subs = el('div', 'fab-sub-container');
    main = el('button', 'fab-main-btn', BUG_ICON);
    main.type = 'button';
    main.dataset.toolId = 'issue-report';
    main.setAttribute('aria-label', 'Report a Bug');
    var mainTip = el('span', 'fab-tooltip');
    mainTip.textContent = 'Report a Bug';
    main.appendChild(mainTip);
    bar.appendChild(main);
    bar.appendChild(subs);
    document.body.appendChild(bar);
    addTool('issue-list', 'Reported issues', LIST_ICON, function () {
      openPanel('list');
    });
    main.addEventListener('click', function (e) {
      e.stopPropagation();
      openPanel('form');
    });

    // The panel.
    panel = el('div', 'issue-orb-panel');
    panel.id = 'issue-orb-panel';
    panel.setAttribute('role', 'dialog');
    panel.setAttribute('aria-live', 'polite');
    panel.setAttribute('aria-label', 'Report an issue');
    panel.setAttribute('data-visible', 'false');
    panel.innerHTML =
      '<button type="button" class="issue-orb-close" aria-label="Close">&times;</button>' +
      '<div class="issue-view-form">' +
      '<h4>Report an issue</h4>' +
      '<form class="issue-report-form" novalidate>' +
      '<div class="issue-report-kinds" role="group" aria-label="Kind of issue">' +
      KINDS.map(function (k, i) {
        return (
          '<button type="button" class="issue-report-kind" data-kind="' + k.value + '" aria-pressed="' + (i === 0) + '">' +
          '<span class="issue-report-kind-label">' + k.label + '</span>' +
          '<span class="issue-report-kind-hint">' + k.hint + '</span></button>'
        );
      }).join('') +
      '</div>' +
      '<label for="issue-report-title">Title</label>' +
      '<input id="issue-report-title" type="text" class="issue-report-title" maxlength="100" required autocomplete="off">' +
      '<label for="issue-report-description">Description</label>' +
      '<textarea id="issue-report-description" class="issue-report-description" maxlength="2000" rows="4" required></textarea>' +
      '<div class="issue-report-shot">' +
      '<button type="button" class="issue-report-attach">Attach screenshot</button>' +
      '<img class="issue-report-thumb" alt="Screenshot attachment preview" hidden>' +
      '<span class="issue-report-shot-status">or paste one &middot; optional</span>' +
      '<button type="button" class="issue-report-remove" hidden>Remove</button>' +
      '<input type="file" class="issue-report-file" accept="image/png,image/jpeg,image/gif,image/webp" hidden>' +
      '</div>' +
      '<p class="issue-report-error" role="alert" hidden></p>' +
      '<p class="issue-report-note">The equation in the editor, your browser and the app version are attached automatically. No account needed.</p>' +
      '<button type="submit" class="issue-report-submit">Send report</button>' +
      '</form>' +
      '<p class="issue-report-thanks" hidden></p>' +
      '<button type="button" class="issue-orb-link" data-go="list">See what others reported</button>' +
      '</div>' +
      '<div class="issue-view-list" hidden>' +
      '<h4>Reported issues</h4>' +
      '<div class="issue-list"></div>' +
      '<button type="button" class="issue-orb-link" data-go="form">Report an issue</button>' +
      '</div>';
    document.body.appendChild(panel);

    var form = panel.querySelector('.issue-report-form');
    var kindButtons = Array.prototype.slice.call(panel.querySelectorAll('.issue-report-kind'));
    var titleField = panel.querySelector('.issue-report-title');
    var descField = panel.querySelector('.issue-report-description');
    var submitButton = panel.querySelector('.issue-report-submit');
    var attachBtn = panel.querySelector('.issue-report-attach');
    var removeBtn = panel.querySelector('.issue-report-remove');
    var fileInput = panel.querySelector('.issue-report-file');
    var thumb = panel.querySelector('.issue-report-thumb');
    var shotStatus = panel.querySelector('.issue-report-shot-status');
    var thanks = panel.querySelector('.issue-report-thanks');
    errorBox = panel.querySelector('.issue-report-error');

    function kind() {
      return KINDS.filter(function (k) {
        return k.value === selectedKind;
      })[0];
    }

    function paintKinds() {
      kindButtons.forEach(function (b) {
        var k = KINDS.filter(function (x) {
          return x.value === b.dataset.kind;
        })[0];
        var active = b.dataset.kind === selectedKind;
        b.setAttribute('aria-pressed', String(active));
        b.style.borderColor = active ? k.color : '';
        b.style.color = active ? k.color : '';
      });
      var cur = kind();
      submitButton.style.background = cur.color;
      submitButton.style.color = cur.ink;
      // Placeholders only: never overwrite what the reporter already typed.
      titleField.placeholder = PLACEHOLDERS[selectedKind].title;
      descField.placeholder = PLACEHOLDERS[selectedKind].body;
    }

    kindButtons.forEach(function (b) {
      b.addEventListener('click', function () {
        selectedKind = b.dataset.kind;
        paintKinds();
      });
    });
    paintKinds();

    function paintShot() {
      var has = Boolean(shot);
      thumb.hidden = !has;
      if (has) thumb.src = shot.dataUrl;
      else thumb.removeAttribute('src');
      removeBtn.hidden = !has;
      attachBtn.hidden = has;
      shotStatus.textContent = has ? 'Screenshot attached' : 'or paste one · optional';
    }

    function acceptShot(file) {
      showError('');
      if (!file || !/^image\/(png|jpeg|gif|webp)$/.test(file.type)) {
        showError('Screenshots must be images (PNG, JPEG, GIF or WebP).');
        return;
      }
      var fr = new FileReader();
      fr.onload = function () {
        var url = String(fr.result);
        if (url.length > MAX_SHOT_CHARS) {
          showError('That image is over ' + MAX_SHOT_TEXT + '. Try a smaller screenshot.');
          return;
        }
        shot = { name: file.name || 'pasted image', dataUrl: url };
        paintShot();
      };
      fr.onerror = function () {
        showError('That image could not be read.');
      };
      fr.readAsDataURL(file);
    }

    attachBtn.addEventListener('click', function () {
      fileInput.click();
    });
    removeBtn.addEventListener('click', function () {
      shot = null;
      paintShot();
    });
    fileInput.addEventListener('change', function (e) {
      var f = e.target.files && e.target.files[0];
      if (f) acceptShot(f);
      e.target.value = '';
    });
    panel.addEventListener('paste', function (e) {
      var items = (e.clipboardData && e.clipboardData.items) || [];
      for (var i = 0; i < items.length; i++) {
        if (items[i].type && items[i].type.indexOf('image/') === 0) {
          var f = items[i].getAsFile();
          if (f) {
            e.preventDefault();
            acceptShot(f);
            return;
          }
        }
      }
    });
    paintShot();

    // The page's own key and clipboard handlers (the equation editor) must not see what is typed
    // here.
    ['keydown', 'keyup', 'keypress', 'paste', 'copy', 'cut', 'beforeinput', 'compositionstart', 'compositionend'].forEach(
      function (ev) {
        panel.addEventListener(ev, function (e) {
          e.stopPropagation();
        });
      }
    );

    panel.querySelector('.issue-orb-close').addEventListener('click', function (e) {
      e.stopPropagation();
      setVisible(false);
    });
    Array.prototype.forEach.call(panel.querySelectorAll('.issue-orb-link'), function (b) {
      b.addEventListener('click', function () {
        showView(b.dataset.go);
      });
    });
    panel.addEventListener('click', function (e) {
      e.stopPropagation();
    });

    form.addEventListener('submit', function (e) {
      e.preventDefault();
      if (busy) return;
      var title = titleField.value.trim();
      var desc = descField.value.trim();
      if (!title) {
        showError('Add a short title.');
        titleField.focus();
        return;
      }
      if (!desc) {
        showError('Add a description.');
        descField.focus();
        return;
      }
      showError('');
      busy = true;
      submitButton.disabled = true;
      submitButton.textContent = 'Submitting...';
      var ctx = api();
      var latex = '';
      var version = '';
      try {
        latex = typeof ctx.latex === 'function' ? ctx.latex() : '';
        version = typeof ctx.version === 'function' ? ctx.version() : '';
      } catch (_) {}
      fetch('/api/issue-reports', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          title: title,
          description: desc,
          kind: selectedKind,
          screenshot: shot ? shot.dataUrl : undefined,
          latex: latex,
          browser: navigator.userAgent,
          pageUrl: window.location.href,
          appVersion: version,
        }),
      })
        .then(function (r) {
          return r
            .json()
            .catch(function () {
              return {};
            })
            .then(function (data) {
              if (!r.ok) throw new Error(data.error || 'The server answered ' + r.status + '.');
              return data;
            });
        })
        .then(function () {
          form.reset();
          selectedKind = KINDS[0].value;
          shot = null;
          paintKinds();
          paintShot();
          listLoaded = false;
          form.hidden = true;
          thanks.hidden = false;
          thanks.textContent = 'Thank you! Your report has been submitted.';
          window.setTimeout(function () {
            setVisible(false);
            form.hidden = false;
            thanks.hidden = true;
          }, 2200);
          document.dispatchEvent(new CustomEvent('equashion:report-sent'));
        })
        .catch(function (err) {
          // The text and the screenshot stay, so the reporter can try again.
          showError((err && err.message ? err.message : 'The report could not be sent.') + ' Please try again.');
        })
        .then(function () {
          busy = false;
          submitButton.disabled = false;
          submitButton.textContent = 'Send report';
        });
    });

    // Closing: Escape, a click anywhere else on the page.
    document.addEventListener('keydown', function (e) {
      if (e.key === 'Escape' && isVisible()) setVisible(false);
    });
    document.addEventListener('click', function (e) {
      if (isVisible() && !panel.contains(e.target) && !bar.contains(e.target)) setVisible(false);
    });
    // eframe handles pointer events on the canvas itself, which stops them reaching `document`
    // clicks above; pointerdown on the window sees them first.
    window.addEventListener(
      'pointerdown',
      function (e) {
        if (isVisible() && !panel.contains(e.target) && !bar.contains(e.target)) setVisible(false);
      },
      true
    );
  }

  // ---- the list -----------------------------------------------------------------------------

  function loadList() {
    var host = panel.querySelector('.issue-list');
    if (listLoaded) return;
    host.innerHTML = '<p class="issue-list-empty">Loading...</p>';
    fetch('/api/issue-reports', { headers: { accept: 'application/json' } })
      .then(function (r) {
        return r
          .json()
          .catch(function () {
            return {};
          })
          .then(function (data) {
            if (!r.ok) throw new Error(data.error || 'The server answered ' + r.status + '.');
            return data;
          });
      })
      .then(function (data) {
        listLoaded = true;
        paintList(host, data.reports || []);
      })
      .catch(function (err) {
        host.innerHTML = '';
        var p = el('p', 'issue-report-error');
        p.textContent = 'The list could not be loaded: ' + (err && err.message ? err.message : 'the network is unavailable.');
        host.appendChild(p);
        var retry = el('button', 'issue-orb-link');
        retry.type = 'button';
        retry.textContent = 'Try again';
        retry.addEventListener('click', loadList);
        host.appendChild(retry);
      });
  }

  function paintList(host, reports) {
    host.innerHTML = '';
    if (!reports.length) {
      var p = el('p', 'issue-list-empty');
      p.textContent = 'Nothing has been reported yet.';
      host.appendChild(p);
      return;
    }
    var mine = voted();
    reports.sort(function (a, b) {
      return (b.votes || 0) - (a.votes || 0);
    });
    reports.forEach(function (r) {
      var id = String(r.id);
      var item = el('div', 'issue-item');
      item.dataset.issueId = id;
      var vote = el('button', 'issue-vote');
      vote.type = 'button';
      var has = mine.indexOf(id) >= 0;
      vote.setAttribute('aria-pressed', String(has));
      vote.disabled = has;
      vote.setAttribute('aria-label', (has ? 'Voted, ' : 'Vote, ') + (r.votes || 0) + ' votes');
      vote.title = has ? 'You voted for this' : 'This matters to me too';
      var count = el('span', 'issue-vote-count');
      count.textContent = String(r.votes || 0);
      vote.appendChild(count);
      vote.appendChild(text(has ? 'Voted' : 'Vote'));
      vote.addEventListener('click', function () {
        castVote(id, vote, count);
      });
      var body = el('div', 'issue-item-body');
      var title = el('div', 'issue-item-title');
      title.textContent = r.title;
      var meta = el('div', 'issue-item-meta');
      meta.appendChild(text((KIND_LABELS[r.kind] || 'Bug') + ' · ' + (STATUS_LABELS[r.status] || 'Open')));
      if (r.screenshotUrl) {
        meta.appendChild(text(' · '));
        var a = el('a');
        a.href = r.screenshotUrl;
        a.target = '_blank';
        a.rel = 'noopener';
        a.textContent = 'Screenshot';
        meta.appendChild(a);
      }
      var desc = el('div', 'issue-item-desc');
      var d = String(r.description || '');
      desc.textContent = d.length > 240 ? d.slice(0, 240) + '...' : d;
      body.appendChild(title);
      body.appendChild(meta);
      body.appendChild(desc);
      item.appendChild(vote);
      item.appendChild(body);
      host.appendChild(item);
    });
  }

  function castVote(id, button, count) {
    if (voting[id] || voted().indexOf(id) >= 0) return;
    voting[id] = true;
    button.disabled = true;
    fetch('/api/issue-reports/vote', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ id: id }),
    })
      .then(function (r) {
        return r
          .json()
          .catch(function () {
            return {};
          })
          .then(function (data) {
            if (!r.ok) throw new Error(data.error || 'The server answered ' + r.status + '.');
            return data;
          });
      })
      .then(function (data) {
        remember(id);
        count.textContent = String(data.votes);
        button.setAttribute('aria-pressed', 'true');
        button.setAttribute('aria-label', 'Voted, ' + data.votes + ' votes');
        button.lastChild.textContent = 'Voted';
        button.title = 'You voted for this';
        // Most voted first, as the server sends them.
        listLoaded = false;
      })
      .catch(function (err) {
        button.disabled = false;
        button.title = 'Your vote was not counted: ' + (err && err.message ? err.message : 'try again');
      })
      .then(function () {
        delete voting[id];
      });
  }

  var pendingTools = [];

  function start() {
    if (document.getElementById('issue-orb-panel')) return;
    build();
    pendingTools.splice(0).forEach(function (t) {
      addTool(t[0], t[1], t[2], t[3]);
    });
  }

  // Other page scripts (coffee.js) add their button to the same column and can close the panel.
  window.equashionReportUi = {
    open: openPanel,
    close: function () {
      if (panel && isVisible()) setVisible(false);
    },
    // Tools added before the column is built wait for it.
    addTool: function (id, label, icon, onClick) {
      if (subs) addTool(id, label, icon, onClick);
      else pendingTools.push([id, label, icon, onClick]);
    },
  };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start);
  else start();
})();
