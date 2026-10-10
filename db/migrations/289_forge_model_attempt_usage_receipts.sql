begin;

alter table forge_model_generation_attempt
    add column if not exists usage_settled boolean not null default false,
    add column if not exists usage_known boolean not null default false,
    add column if not exists usage_tokens_input bigint,
    add column if not exists usage_tokens_output bigint,
    add column if not exists usage_cost_usd numeric,
    add column if not exists usage_session_id text;

do $$
begin
    if not exists (
        select 1 from pg_constraint
         where conname = 'forge_model_generation_attempt_usage_shape'
           and conrelid = 'forge_model_generation_attempt'::regclass
    ) then
        alter table forge_model_generation_attempt
            add constraint forge_model_generation_attempt_usage_shape check (
                (not usage_settled and not usage_known
                    and usage_tokens_input is null and usage_tokens_output is null
                    and usage_cost_usd is null and usage_session_id is null)
                or (usage_settled and not usage_known
                    and usage_tokens_input is null and usage_tokens_output is null
                    and usage_cost_usd is null and usage_session_id is null)
                or (usage_settled and usage_known
                    and usage_tokens_input >= 0 and usage_tokens_output >= 0
                    and usage_cost_usd >= 0 and usage_session_id is not null
                    and length(btrim(usage_session_id)) > 0)
            );
    end if;
end $$;

comment on column forge_model_generation_attempt.usage_settled is
    'Whether this stable attempt identity has an explicit measured or unknown usage settlement.';
comment on column forge_model_generation_attempt.usage_known is
    'True only when the settled receipt contains a measured vendor usage reading.';

commit;
