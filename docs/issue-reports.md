# Issue reports

The page's **Report an issue** form is modelled on raSHio's, minus the sign-in. Anyone can send a
report. It POSTs JSON to `/api/issue-reports` on the same origin:

| Field | Meaning |
|---|---|
| `title` | 1 to 100 characters |
| `description` | 1 to 2000 characters |
| `kind` | `bug`, `quirk` or `enhancement` (the form's Bug, Quirk, Improve) |
| `screenshot` | optional `data:image/(png\|jpeg\|gif\|webp);base64,...`, at most 1.4 million characters (about 1 MB of image) |
| `latex` | the equation in the editor |
| `browser`, `pageUrl`, `appVersion` | context for reproducing it |

Answers: `201 {"success":true,"id":...}` or `4xx {"error":"..."}` with the reason in words, which
the page shows. Limits are checked on the page and again on the server.

## Where reports go

- **Local** (`cargo xtask serve`, `cargo xtask smoke`): each report is written to
  `target/issue-reports/<millis>.json`. Nothing leaves the machine. This is the only place
  reports go until the site is deployed with the function below.
- **Deployed**: `functions/api/issue-reports.js` is a Cloudflare Pages Function that stores the
  report in D1. It needs a D1 database bound as `EQUASHION_DB` with
  `migrations/0001_issue_reports.sql` applied, and optionally a `REPORT_SALT` secret. Nothing is
  deployed or created from this repository; that is a separate step.

There is no account to rate-limit by, so the function limits each sender (a salted SHA-256 of
`CF-Connecting-IP`, never returned) to five reports an hour.

## Reading them

```
wrangler d1 execute equashion --command \
  "SELECT created_at, kind, status, title, description, page_url FROM issue_reports ORDER BY created_at DESC"
```

## What is not here (compared with raSHio)

raSHio also has a public issues list with votes, withdraw, status triage for admins and shared
screenshots. Those need accounts and were left out: this page has none.

## Testing

`apps/equashion-web/tests/showcase.rs` drives the form natively (a filled form queues one
submission with the equation and context, an empty one is refused, a failed send keeps the text,
only images attach). `tests/browser/smoke.sh` posts a real report through `xtask serve`. The file
picker and image paste need a real user gesture and are not driven headless.
