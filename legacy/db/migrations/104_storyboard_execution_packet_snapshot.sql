-- ENG-FORGE-V5-03R — durable executable packet snapshot.
--
-- Git remains the planned packet source; once a story is admitted for Forge
-- execution, Neon carries the exact executable verification contract so the
-- worker does not depend on a freshly-pulled local markdown file.

ALTER TABLE storyboard_story
  ADD COLUMN IF NOT EXISTS test_mode text;

ALTER TABLE storyboard_story
  ADD COLUMN IF NOT EXISTS assay_commands text;

ALTER TABLE storyboard_story
  ADD COLUMN IF NOT EXISTS packet_sha text;
