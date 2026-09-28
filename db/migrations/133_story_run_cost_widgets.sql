-- storyboard_story_run: separate Forge widget units (cost_widgets) from real
-- settled USD (cost_usd).
--
-- Model: tokens_input/output = provider textual metering; cost_widgets = Forge's
-- standardized, model-relative consumption quantity (model_weight x elapsed
-- minutes) that calibration regresses on; cost_usd is RESERVED for actual vendor
-- USD (rare — the vendor invoices late). cost_source records which quantity a row
-- actually carries ('widgets' | 'vendor').
--
-- Historical rows: the fallback writer populated cost_usd with WIDGETS (no real
-- USD was ever available in real time). Copy those into cost_widgets and label
-- them widgets. Non-destructive: cost_usd is left untouched so any genuine
-- vendor row is never altered.
alter table storyboard_story_run
  add column if not exists cost_widgets numeric,
  add column if not exists cost_source text;

update storyboard_story_run
  set cost_widgets = cost_usd,
      cost_source = coalesce(cost_source, 'widgets')
  where cost_usd is not null and cost_widgets is null;
