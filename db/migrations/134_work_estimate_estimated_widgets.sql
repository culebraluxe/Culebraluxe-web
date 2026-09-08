-- work_estimate: the forecast's cost column is Forge WIDGET units (the same
-- standardized, model-relative unit as storyboard_story_run.cost_widgets), not
-- USD. Rename for honesty so forecast and actual both speak "widgets". The
-- token count stays the provider metering field; USD is reserved for actual
-- settlement elsewhere (rare).
alter table work_estimate rename column estimated_cost_usd to estimated_widgets;
