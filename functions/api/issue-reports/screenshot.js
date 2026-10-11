// GET /api/issue-reports/screenshot?id=<report id>: the screenshot attached to a report, as the
// image it was sent as. The id is a random UUID that only the GitHub issue's link carries.
import { json } from '../issue-reports.js';

export const onRequestGet = async ({ request, env }) => {
  const id = new URL(request.url).searchParams.get('id') || '';
  if (!/^[0-9a-f-]{36}$/.test(id) || !env.EQUASHION_DB) return json({ error: 'Not found.' }, 404);
  const row = await env.EQUASHION_DB.prepare('SELECT screenshot FROM report_extras WHERE report_id = ?1').bind(id).first();
  const m = /^data:(image\/(?:png|jpeg|gif|webp));base64,(.*)$/s.exec(row?.screenshot || '');
  if (!m) return json({ error: 'Not found.' }, 404);
  const bytes = Uint8Array.from(atob(m[2]), (c) => c.charCodeAt(0));
  return new Response(bytes, {
    headers: { 'content-type': m[1], 'cache-control': 'public, max-age=86400', 'x-content-type-options': 'nosniff' },
  });
};
