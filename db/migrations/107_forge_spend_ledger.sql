-- Forge spend vision — cost ledger on the existing run contract.
--
-- Tokens/cost are harness-observed facts, never model self-report. Null =
-- unmeasured (older runs, model-free Assay, harnesses without metering).
-- model_used is the exact provider/model the run executed under, enforced
-- per-run by the harness (OpenCode --model pin, dsh --patch overlay).

alter table storyboard_story_run
  add column if not exists model_used text,
  add column if not exists tokens_input integer,
  add column if not exists tokens_output integer,
  add column if not exists cost_usd numeric(12, 6);
