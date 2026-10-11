# Issue reports

The page's **Report an issue** form is modelled on raSHio's, minus the sign-in. Anyone can send a
report, see what has been reported and vote.

## What the page does

- **Send**: kind (Bug, Quirk, Improve), a title (100 characters), a description (2000) and an
  optional screenshot (a file or a pasted image, 1 MB at most). The report also carries the
  equation in the editor, the browser, the page address and the app version.
- **Reported so far**: the public list under the form, most voted first, with each issue's kind and
  status. It loads when the form first opens and again after a send.
- **Vote**: one per visitor per issue. The page remembers your votes in this browser
  (`localStorage`), the server counts one per sender.

## API

| Request | Body | Answer |
|---|---|---|
| `POST /api/issue-reports` | `title`, `description`, `kind` (`bug`, `quirk`, `enhancement`), optional `screenshot` (`data:image/(png\|jpeg\|gif\|webp);base64,...`, at most 1.4 million characters), `latex`, `browser`, `pageUrl`, `appVersion` | `201 {"success":true,"id":"<issue number>"}` or `4xx {"error":"..."}` |
| `GET /api/issue-reports` | | `{"reports":[{id, kind, title, description, status, votes, createdAt, screenshotUrl}]}` |
| `POST /api/issue-reports/vote` | `{"id": "<issue number>"}` | `{"votes": n}` |
| `GET /api/issue-reports/screenshot?id=<uuid>` | | the image |

`status` is `open`, `in_progress`, `fixed` or `deferred`. Errors carry the reason in words, which
the page shows. Limits are checked on the page and again on the server.

## Where reports live

- **Deployed**: each report becomes an **issue on GitHub** (`GITHUB_REPO`, default
  `shuff57/equaSHion`), labelled `from-page` and with its kind. The list is read back from GitHub.
  So **triage is GitHub**: close an issue = *fixed*, close it as "not planned" = *deferred*, add the
  label `in progress` = *in progress*. There is no separate admin page.
  A small D1 database holds only what GitHub cannot: the votes, the screenshots (GitHub has no
  upload API for issues) and the rate-limit record.
- **Local** (`cargo xtask serve`, `cargo xtask smoke`): reports are written to
  `target/issue-reports/<millis>.json` and votes to `votes.txt` beside them. Nothing leaves the
  machine; the list and the vote work the same way. This is the only place reports go until the
  site is deployed with the functions below.

## Deploying (not done from this repository)

`functions/api/issue-reports.js`, `functions/api/issue-reports/vote.js` and
`functions/api/issue-reports/screenshot.js` are Cloudflare Pages Functions. They need:

| Binding | What |
|---|---|
| `EQUASHION_DB` | a D1 database with `migrations/0001_issue_reports.sql` applied |
| `GITHUB_TOKEN` (secret) | a fine-grained token with **Issues: read and write** on the one repo |
| `GITHUB_REPO` (optional) | `owner/name`, default `shuff57/equaSHion` |
| `REPORT_SALT` (secret, optional) | salt for the sender hash |

There is no account to rate-limit by, so a sender (a salted SHA-256 of `CF-Connecting-IP`, never
returned) is limited to five reports an hour, and one vote per issue.

Privacy: the public list shows only the reporter's own title and description and the status. The
equation, browser and page address are in the GitHub issue, so keep the repo's issues in mind when
choosing whether it is public.

## Testing

- `apps/equashion-web/tests/showcase.rs`: the form, the list, voting and failure states, natively.
- `tests/functions/issue-reports.test.mjs` (`bun test tests/functions/`): the functions against a
  fake D1 and a fake GitHub (status mapping, labels, validation, the rate limit, one vote each,
  what the list hides).
- `tests/browser/smoke.sh`: posts a real report through `xtask serve`, finds it in the list and
  votes for it. The file picker and image paste need a real user gesture and are not driven
  headless.
