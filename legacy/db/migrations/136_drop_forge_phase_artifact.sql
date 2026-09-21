-- Drop forge_phase_artifact. The phase model is: the STORY RUN row is the phase
-- SUMMARY (the forward handoff fed to the next role — raw output in notes, model/
-- cost/verdict on the row), and any discrete work products that need independent
-- life are M rows in forge_tool_artifact (already exists). A per-phase fat row
-- duplicating notes + a findings array was redundant with both.
drop table if exists forge_phase_artifact;
