-- Issue reports from the equaSHion page. Anyone can send one: there are no accounts, so the
-- only identity is a salted hash of the sender's address, kept to rate-limit and never shown.
-- Read them with: wrangler d1 execute equashion --command "SELECT * FROM issue_reports ORDER BY created_at DESC"

CREATE TABLE IF NOT EXISTS issue_reports (
  id           TEXT PRIMARY KEY,
  kind         TEXT NOT NULL CHECK (kind IN ('bug','quirk','enhancement')),
  title        TEXT NOT NULL,
  description  TEXT NOT NULL,
  status       TEXT NOT NULL DEFAULT 'open'
                 CHECK (status IN ('open','in_progress','fixed','deferred')),
  latex        TEXT,
  screenshot   TEXT,            -- a data: URL, 1 MB at most
  browser      TEXT,
  page_url     TEXT,
  app_version  TEXT,
  sender_hash  TEXT NOT NULL,
  created_at   TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_issue_reports_status ON issue_reports(status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_issue_reports_sender ON issue_reports(sender_hash, created_at DESC);
