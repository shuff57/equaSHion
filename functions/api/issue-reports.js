// POST /api/issue-reports: file an issue report from the equaSHion page. Cloudflare Pages
// Function with a D1 binding named EQUASHION_DB (see migrations/0001_issue_reports.sql).
//
// No sign-in: anyone can report. What stands in for an account is a rate limit per sender
// (a salted hash of CF-Connecting-IP, never stored in the clear and never returned).
// `xtask serve` answers this same path for local runs by writing the body to
// target/issue-reports/.

const KINDS = ['bug', 'quirk', 'enhancement'];
const TITLE_MAX = 100;
const DESC_MAX = 2000;
const LATEX_MAX = 4000;
const CONTEXT_MAX = 500;
const SHOT_MAX_CHARS = 1_400_000;
const SHOT_PREFIXES = [
  'data:image/png;base64,',
  'data:image/jpeg;base64,',
  'data:image/gif;base64,',
  'data:image/webp;base64,',
];
// Five reports an hour per sender: enough for a real person, a wall for a script.
const LIMIT = 5;
const WINDOW_SECONDS = 3600;

const json = (payload, status = 200) =>
  new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store' },
  });

const clip = (value, max) => String(value ?? '').trim().slice(0, max);

const senderHash = async (request, env) => {
  const ip = request.headers.get('CF-Connecting-IP') || 'unknown';
  const data = new TextEncoder().encode(`${env.REPORT_SALT || 'equashion'}:${ip}`);
  const digest = await crypto.subtle.digest('SHA-256', data);
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, '0')).join('');
};

export const onRequestPost = async ({ request, env }) => {
  if (!env.EQUASHION_DB) return json({ error: 'The report database is not set up.' }, 500);

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
    'SELECT COUNT(*) AS n FROM issue_reports WHERE sender_hash = ?1 AND created_at > ?2',
  ).bind(sender, since).first();
  if ((recent?.n ?? 0) >= LIMIT) {
    return json({ error: 'That is a lot of reports for one hour. Please try again later.' }, 429);
  }

  const id = crypto.randomUUID();
  try {
    await env.EQUASHION_DB.prepare(
      `INSERT INTO issue_reports
         (id, kind, title, description, status, latex, screenshot, browser, page_url, app_version, sender_hash, created_at)
       VALUES (?1, ?2, ?3, ?4, 'open', ?5, ?6, ?7, ?8, ?9, ?10, ?11)`,
    ).bind(
      id,
      kind,
      title,
      description,
      clip(body.latex, LATEX_MAX) || null,
      screenshot,
      clip(body.browser, CONTEXT_MAX) || null,
      clip(body.pageUrl, CONTEXT_MAX) || null,
      clip(body.appVersion, 40) || null,
      sender,
      new Date().toISOString(),
    ).run();
  } catch (error) {
    console.error('Saving an issue report failed', error);
    return json({ error: 'The report could not be saved. Try again.' }, 500);
  }
  return json({ success: true, id }, 201);
};

export const onRequestOptions = () => new Response(null, { status: 204 });
