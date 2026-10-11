-- What the page needs next to the GitHub issues it files. The issue (title, text, status) lives
-- in GitHub; D1 holds only what GitHub cannot: one vote per visitor per issue, the screenshot, and
-- the rate-limit record. There are no accounts, so a visitor is a salted hash of their address,
-- kept to count and never shown.

CREATE TABLE IF NOT EXISTS report_extras (
  report_id     TEXT PRIMARY KEY,       -- random id, used in the screenshot link
  issue_number  INTEGER NOT NULL,
  sender_hash   TEXT NOT NULL,
  screenshot    TEXT,                   -- a data: URL, 1 MB at most
  created_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_report_extras_sender ON report_extras(sender_hash, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_report_extras_issue ON report_extras(issue_number);

CREATE TABLE IF NOT EXISTS report_votes (
  issue_number  INTEGER NOT NULL,
  sender_hash   TEXT NOT NULL,
  created_at    TEXT NOT NULL,
  PRIMARY KEY (issue_number, sender_hash)
);
