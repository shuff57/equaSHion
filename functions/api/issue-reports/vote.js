// POST /api/issue-reports/vote {id}: one vote per visitor per issue. Returns {votes}.
import { json, senderHash } from '../issue-reports.js';

export const onRequestPost = async ({ request, env }) => {
  if (!env.EQUASHION_DB) return json({ error: 'Voting is not set up on this site yet.' }, 500);
  let body;
  try {
    body = await request.json();
  } catch {
    return json({ error: 'That is not a valid vote.' }, 400);
  }
  const number = Number.parseInt(String(body.id ?? ''), 10);
  if (!Number.isSafeInteger(number) || number <= 0) return json({ error: 'Unknown issue.' }, 400);

  const sender = await senderHash(request, env);
  // INSERT OR IGNORE: a second vote from the same visitor changes nothing.
  await env.EQUASHION_DB.prepare(
    'INSERT OR IGNORE INTO report_votes (issue_number, sender_hash, created_at) VALUES (?1, ?2, ?3)',
  ).bind(number, sender, new Date().toISOString()).run();
  const row = await env.EQUASHION_DB.prepare('SELECT COUNT(*) AS n FROM report_votes WHERE issue_number = ?1').bind(number).first();
  return json({ votes: row?.n ?? 0 });
};
