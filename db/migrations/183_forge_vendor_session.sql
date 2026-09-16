-- 183_forge_vendor_session.sql
-- The vendor session pointer lives in the rows, not in a worktree file (Captain, 2026-09-16).
--
-- A lane's session id is a POINTER, and a valuable one: one session serves a whole generation, so the next
-- role resumes the same context instead of rebuilding it — that is real token money. It used to live in
-- `.forge-session.continue` inside the worktree: state on disk, invisible to any query, gone with the
-- machine, and it forced every lane to own a tree to keep it. It lives here instead, keyed to the story and
-- the worker, written by the engine and read back by the next role. The lane stops being the keeper of the
-- fact; the row is.

CREATE TABLE IF NOT EXISTS forge_vendor_session (
  story_id   text        NOT NULL,
  worker_id  text        NOT NULL,
  session_id text,
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (story_id, worker_id)
);

COMMENT ON TABLE forge_vendor_session IS
  'Vendor (opencode) session pointer per story+worker. NULL session_id means the recorded session is dead and the next role starts fresh.';
