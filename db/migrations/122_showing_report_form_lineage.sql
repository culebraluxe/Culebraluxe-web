-- CulebraLuxe
-- Showing Report canonical service facts + explicit Form lineage.
-- Migration: 122_showing_report_form_lineage.sql
--
-- SHOW-RPT enriches the existing Showing record. It does not create a parallel
-- report domain and it does not infer a Showing from Person/Property/latest row.
-- A Form carries an explicit showing_id once its sidecar is created.

begin;

alter table showing
  add column if not exists showing_date date,
  add column if not exists duration text,
  add column if not exists outcome text,
  add column if not exists interest_score smallint,
  add column if not exists follow_up text;

alter table showing drop constraint if exists showing_report_outcome_check;
alter table showing add constraint showing_report_outcome_check
  check (
    outcome is null
    or outcome in ('Interested', 'Second showing', 'Offer expected', 'Not a fit')
  );

alter table showing drop constraint if exists showing_interest_score_check;
alter table showing add constraint showing_interest_score_check
  check (interest_score is null or interest_score between 1 and 5);

alter table document_form_instance
  add column if not exists showing_id uuid
    references showing(id)
    on delete restrict;

create index if not exists idx_document_form_instance_showing
  on document_form_instance(showing_id)
  where showing_id is not null;

alter table document_form_instance
  drop constraint if exists document_form_instance_context_check;

alter table document_form_instance
  add constraint document_form_instance_context_check
  check (
    contract_id is not null
    or showing_id is not null
    or deal_id is not null
    or person_id is not null
    or property_id is not null
  );

comment on column document_form_instance.showing_id is
  'Explicit lineage to the canonical Showing enriched by SHOW-RPT. Never inferred from Person, Property, Deal, or latest row.';
comment on column showing.showing_date is
  'Business date of the Showing. Separate from lifecycle timestamps because the report does not invent a time-of-day.';
comment on column showing.follow_up is
  'Showing Report follow-up / next-action notes.';

commit;
