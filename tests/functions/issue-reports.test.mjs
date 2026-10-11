// Run with: bun test tests/functions/  (or: node --test tests/functions/*.test.mjs)
import test from 'node:test';
import assert from 'node:assert/strict';
import { DETAILS_MARK, issueBody, statusOf, onRequestPost, onRequestGet } from '../../functions/api/issue-reports.js';
import { onRequestPost as vote } from '../../functions/api/issue-reports/vote.js';

// A D1 stand-in: just enough of prepare/bind/first/all/run for these functions.
const fakeDb = () => {
  const rows = { extras: [], votes: new Set() };
  return {
    rows,
    prepare(sql) {
      let args = [];
      const q = {
        bind: (...a) => ((args = a), q),
        first: async () => {
          if (/FROM report_extras WHERE sender_hash/.test(sql)) return { n: rows.extras.filter((r) => r.sender === args[0]).length };
          if (/FROM report_votes WHERE issue_number/.test(sql)) return { n: [...rows.votes].filter((v) => v.startsWith(`${args[0]}:`)).length };
          return null;
        },
        all: async () => ({ results: [] }),
        run: async () => {
          if (/INSERT INTO report_extras/.test(sql)) rows.extras.push({ sender: args[2], issue: args[1] });
          if (/INSERT OR IGNORE INTO report_votes/.test(sql)) rows.votes.add(`${args[0]}:${args[1]}`);
        },
      };
      return q;
    },
  };
};

const post = (body, ip = '1.1.1.1') =>
  new Request('https://x.test/api/issue-reports', {
    method: 'POST',
    headers: { 'CF-Connecting-IP': ip },
    body: JSON.stringify(body),
  });

const good = { title: 'T', description: 'D', kind: 'bug', latex: 'x^2', browser: 'UA', pageUrl: 'http://p', appVersion: '0.1.0' };

test('status comes from the GitHub issue', () => {
  assert.equal(statusOf({ state: 'open', labels: [] }), 'open');
  assert.equal(statusOf({ state: 'open', labels: [{ name: 'in progress' }] }), 'in_progress');
  assert.equal(statusOf({ state: 'closed', state_reason: 'completed', labels: [] }), 'fixed');
  assert.equal(statusOf({ state: 'closed', state_reason: 'not_planned', labels: [] }), 'deferred');
});

test('the issue body keeps the description before the mark and the context after', () => {
  const body = issueBody({ ...good, kind: 'quirk', shotId: 'abc' }, 'https://x.test');
  const [desc, details] = body.split(DETAILS_MARK);
  assert.equal(desc, 'D');
  assert.match(details, /\*\*Kind:\*\* quirk/);
  assert.match(details, /```latex\nx\^2\n```/);
  assert.match(details, /screenshot\?id=abc/);
});

test('a report becomes a GitHub issue with the page label and the kind', async () => {
  const db = fakeDb();
  const calls = [];
  globalThis.fetch = async (url, init) => {
    calls.push({ url, init });
    return new Response(JSON.stringify({ number: 42 }), { status: 201 });
  };
  const res = await onRequestPost({ request: post(good), env: { EQUASHION_DB: db, GITHUB_TOKEN: 't', GITHUB_REPO: 'o/r' } });
  assert.equal(res.status, 201);
  assert.deepEqual(await res.json(), { success: true, id: '42' });
  assert.equal(calls[0].url, 'https://api.github.com/repos/o/r/issues');
  assert.deepEqual(JSON.parse(calls[0].init.body).labels, ['from-page', 'bug']);
  assert.equal(db.rows.extras.length, 1);
});

test('bad reports are refused with a reason and never reach GitHub', async () => {
  let called = false;
  globalThis.fetch = async () => ((called = true), new Response('{}'));
  const env = { EQUASHION_DB: fakeDb(), GITHUB_TOKEN: 't' };
  for (const [patch, text] of [
    [{ title: '' }, 'Add a short title.'],
    [{ description: '' }, 'Add a description.'],
    [{ kind: 'urgent' }, 'The kind must be'],
    [{ title: 'x'.repeat(101) }, '100 characters'],
    [{ screenshot: 'data:text/html;base64,AAAA' }, 'Screenshots must be images'],
  ]) {
    const res = await onRequestPost({ request: post({ ...good, ...patch }), env });
    assert.equal(res.status, 400);
    assert.match((await res.json()).error, new RegExp(text));
  }
  assert.equal(called, false);
});

test('a sender is limited to five reports an hour', async () => {
  globalThis.fetch = async () => new Response(JSON.stringify({ number: 1 }), { status: 201 });
  const env = { EQUASHION_DB: fakeDb(), GITHUB_TOKEN: 't' };
  for (let i = 0; i < 5; i++) assert.equal((await onRequestPost({ request: post(good), env })).status, 201);
  assert.equal((await onRequestPost({ request: post(good), env })).status, 429);
  assert.equal((await onRequestPost({ request: post(good, '2.2.2.2'), env })).status, 201, 'another sender is not limited');
});

test('one vote per visitor per issue', async () => {
  const env = { EQUASHION_DB: fakeDb() };
  const v = (id, ip) =>
    vote({ request: new Request('https://x.test/api/issue-reports/vote', { method: 'POST', headers: { 'CF-Connecting-IP': ip }, body: JSON.stringify({ id }) }), env });
  assert.deepEqual(await (await v('7', '1.1.1.1')).json(), { votes: 1 });
  assert.deepEqual(await (await v('7', '1.1.1.1')).json(), { votes: 1 });
  assert.deepEqual(await (await v('7', '2.2.2.2')).json(), { votes: 2 });
  assert.equal((await v('nope', '1.1.1.1')).status, 400);
});

test('the list shows status and votes and hides the context', async () => {
  globalThis.fetch = async () =>
    new Response(
      JSON.stringify([
        { number: 3, title: 'Closed', body: `Done${DETAILS_MARK}**Browser:** secret`, state: 'closed', state_reason: 'completed', labels: [{ name: 'bug' }], created_at: '2026-01-02T00:00:00Z' },
        { number: 4, title: 'Open', body: 'Open it', state: 'open', labels: [{ name: 'enhancement' }, { name: 'in progress' }], created_at: '2026-01-03T00:00:00Z' },
      ]),
    );
  const res = await onRequestGet({ request: new Request('https://x.test/api/issue-reports'), env: { EQUASHION_DB: fakeDb() } });
  const { reports } = await res.json();
  assert.deepEqual(reports.map((r) => [r.id, r.status, r.kind, r.description]), [
    ['4', 'in_progress', 'enhancement', 'Open it'],
    ['3', 'fixed', 'bug', 'Done'],
  ]);
  assert.ok(!JSON.stringify(reports).includes('secret'));
});
