// /api/issue-reports: the equaSHion page's issue reports. Cloudflare Pages Function.
//
//   GET  /api/issue-reports        the public list: reported issues with status and votes
//   POST /api/issue-reports        file a report: becomes a GitHub issue
//
// Each report is a GitHub issue in GITHUB_REPO (default shuff57/equaSHion), made with the secret
// GITHUB_TOKEN (a fine-grained token with Issues: read and write on that one repo). GitHub is
// where reports are triaged: close = fixed, close as "not planned" = deferred, label
// "in progress" = in progress. D1 (binding EQUASHION_DB, migrations/0001_issue_reports.sql) keeps
// the votes, the screenshots and the rate-limit record. No sign-in: a visitor is a salted hash
// of CF-Connecting-IP (secret REPORT_SALT), never returned.
//
// `xtask serve` answers the same paths locally from target/issue-reports/.

export const KINDS = ['bug', 'quirk', 'enhancement'];
const TITLE_MAX = 100;
const DESC_MAX = 2000;
const LATEX_MAX = 4000;
const CONTEXT_MAX = 500;
const SHOT_MAX_CHARS = 1_400_000;
const SHOT_PREFIXES = ['data:image/png;base64,', 'data:image/jpeg;base64,', 'data:image/gif;base64,', 'data:image/webp;base64,'];
// Five reports an hour per sender: enough for a real person, a wall for a script.
const LIMIT = 5;
const WINDOW_SECONDS = 3600;
// Everything the page files carries this label, so the list is only the page's own issues.
export const PAGE_LABEL = 'from-page';
// The body is the description, then this line, then the details only maintainers need.
export const DETAILS_MARK = '\n\n---\n';

export const json = (payload, status = 200, headers = {}) =>
  new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store', ...headers },
  });

export const clip = (value, max) => String(value ?? '').trim().slice(0, max);

export const senderHash = async (request, env) => {
  const ip = request.headers.get('CF-Connecting-IP') || 'unknown';
  const data = new TextEncoder().encode(`${env.REPORT_SALT || 'equashion'}:${ip}`);
  const digest = await crypto.subtle.digest('SHA-256', data);
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, '0')).join('');
};

export const repoOf = (env) => env.GITHUB_REPO || 'shuff57/equaSHion';

export const github = (env, path, init = {}) =>
  fetch(`https://api.github.com/repos/${repoOf(env)}${path}`, {
    ...init,
    headers: {
      accept: 'application/vnd.github+json',
      'user-agent': 'equashion-issue-reports',
      'x-github-api-version': '2022-11-28',
      ...(env.GITHUB_TOKEN ? { authorization: `Bearer ${env.GITHUB_TOKEN}` } : {}),
      ...(init.body ? { 'content-type': 'application/json' } : {}),
    },
  });

// open / in_progress / fixed / deferred, from a GitHub issue.
export const statusOf = (issue) => {
  if (issue.state === 'closed') return issue.state_reason === 'not_planned' ? 'deferred' : 'fixed';
  const names = (issue.labels || []).map((l) => (typeof l === 'string' ? l : l.name));
  return names.includes('in progress') ? 'in_progress' : 'open';
};

const kindOf = (issue) => {
  const names = (issue.labels || []).map((l) => (typeof l === 'string' ? l : l.name));
  return KINDS.find((k) => names.includes(k)) || 'bug';
};

// The description, the mark, then the details only maintainers need.
export const issueBody = (r, origin) => {
  const details = [`**Kind:** ${r.kind}`];
  if (r.latex) details.push('**Equation:**', '```latex', r.latex.replace(/```/g, "'''"), '```');
  if (r.browser) details.push(`**Browser:** ${r.browser}`);
  if (r.pageUrl) details.push(`**Page:** ${r.pageUrl}`);
  if (r.appVersion) details.push(`**Version:** ${r.appVersion}`);
  if (r.shotId) details.push(`**Screenshot:** ${origin}/api/issue-reports/screenshot?id=${r.shotId}`);
  details.push('', '_Filed from the equaSHion page._');
  return `${r.description}${DETAILS_MARK}${details.join(String.fromCharCode(10))}`;
};

// GET: the public list. Only the reporter's own words and the status leave this function; the
// browser, page and equation stay in the issue.
export const onRequestGet = async ({ request, env }) => {
  const url = new URL(request.url);
  const res = await github(env, `/issues?state=all&labels=${PAGE_LABEL}&per_page=100&sort=created&direction=desc`);
  if (!res.ok) return json({ error: 'The list is unavailable right now.' }, 502);
  const issues = (await res.json()).filter((i) => !i.pull_request);

  const votes = new Map();
  if (env.EQUASHION_DB) {
    const { results } = await env.EQUASHION_DB.prepare(
      'SELECT issue_number, COUNT(*) AS n FROM report_votes GROUP BY issue_number',
    ).all();
    for (const row of results || []) votes.set(row.issue_number, row.n);
  }
  const shots = new Map();
  if (env.EQUASHION_DB) {
    const { results } = await env.EQUASHION_DB.prepare(
      'SELECT report_id, issue_number FROM report_extras WHERE screenshot IS NOT NULL',
    ).all();
    for (const row of results || []) shots.set(row.issue_number, row.report_id);
  }

  const reports = issues
    .map((i) => ({
      id: String(i.number),
      kind: kindOf(i),
      title: i.title,
      description: String(i.body || '').split(DETAILS_MARK)[0],
      status: statusOf(i),
      votes: votes.get(i.number) || 0,
      createdAt: i.created_at,
      screenshotUrl: shots.has(i.number) ? `${url.origin}/api/issue-reports/screenshot?id=${shots.get(i.number)}` : null,
    }))
    .sort((a, b) => b.votes - a.votes || (a.createdAt < b.createdAt ? 1 : -1));
  return json({ reports }, 200, { 'cache-control': 'public, max-age=30' });
};

// POST: file a report.
export const onRequestPost = async ({ request, env }) => {
  if (!env.EQUASHION_DB || !env.GITHUB_TOKEN) return json({ error: 'Reporting is not set up on this site yet.' }, 500);

  let body;
  try {
    body = await request.json();
  } catch {
    return json({ error: 'That is not a valid report.' }, 400);
  }

  const title = clip(body.title, TITLE_MAX + 1);
  const description = clip(body.description, DESC_MAX + 1);
  const kind = clip(body.kind, 20);
  if (!title) return json({ error: 'Add a short title.' }, 400);
  if (title.length > TITLE_MAX) return json({ error: `The title must be ${TITLE_MAX} characters or fewer.` }, 400);
  if (!description) return json({ error: 'Add a description.' }, 400);
  if (description.length > DESC_MAX) return json({ error: `The description must be ${DESC_MAX} characters or fewer.` }, 400);
  if (!KINDS.includes(kind)) return json({ error: 'The kind must be bug, quirk or enhancement.' }, 400);

  let screenshot = null;
  if (body.screenshot) {
    const s = String(body.screenshot);
    if (!SHOT_PREFIXES.some((p) => s.startsWith(p))) {
      return json({ error: 'Screenshots must be images (PNG, JPEG, GIF or WebP).' }, 400);
    }
    if (s.length > SHOT_MAX_CHARS) return json({ error: 'That image is over 1 MB. Try a smaller screenshot.' }, 400);
    screenshot = s;
  }

  const sender = await senderHash(request, env);
  const since = new Date(Date.now() - WINDOW_SECONDS * 1000).toISOString();
  const recent = await env.EQUASHION_DB.prepare(
    'SELECT COUNT(*) AS n FROM report_extras WHERE sender_hash = ?1 AND created_at > ?2',
  ).bind(sender, since).first();
  if ((recent?.n ?? 0) >= LIMIT) {
    return json({ error: 'That is a lot of reports for one hour. Please try again later.' }, 429);
  }

  const shotId = screenshot ? crypto.randomUUID() : null;
  const origin = new URL(request.url).origin;
  const gh = await github(env, '/issues', {
    method: 'POST',
    body: JSON.stringify({
      title,
      body: issueBody({
        description,
        kind,
        latex: clip(body.latex, LATEX_MAX),
        browser: clip(body.browser, CONTEXT_MAX),
        pageUrl: clip(body.pageUrl, CONTEXT_MAX),
        appVersion: clip(body.appVersion, 40),
        shotId,
      }, origin),
      labels: [PAGE_LABEL, kind],
    }),
  });
  if (!gh.ok) {
    console.error('GitHub refused the issue', gh.status, await gh.text().catch(() => ''));
    return json({ error: 'The report could not be filed. Try again.' }, 502);
  }
  const issue = await gh.json();

  try {
    await env.EQUASHION_DB.prepare(
      'INSERT INTO report_extras (report_id, issue_number, sender_hash, screenshot, created_at) VALUES (?1, ?2, ?3, ?4, ?5)',
    ).bind(shotId || crypto.randomUUID(), issue.number, sender, screenshot, new Date().toISOString()).run();
  } catch (error) {
    // The issue exists; only the screenshot and the rate-limit record were lost.
    console.error('Saving report extras failed', error);
  }
  return json({ success: true, id: String(issue.number) }, 201);
};

export const onRequestOptions = () => new Response(null, { status: 204 });
