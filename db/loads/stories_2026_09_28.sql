-- OPEN WORK FROM THE 2026-09-28 SESSION, as storyboard stories. Detail: docs/agent/HANDOFF-2026-09-28-claude.md.
-- Insert-only: a story id that already exists is left exactly as it is. Safe to run again.
--
--   ./rust/target/debug/cli db-tool apply db/loads/stories_2026_09_28.sql prod     (from the repo root)

begin;

insert into storyboard_story (id, workstream, title, priority, status, goal, scope, acceptance_criteria, dependencies)
values
('MEDIA-CARD-PROD-01', 'PUBLIC', 'Card-size listing photos live in PROD', 'Critical', 'Ready',
 'Listing cards load a ~200 KB card copy instead of the 1.6-2.7 MB web copy; public photos cache for a year.',
 'Code shipped (fe400426). PROD: apply db/migrations/252_media_card_copy.sql, then run `cli media-cards prod` until it reports 0, then deploy. The migration must precede the deploy.',
 'Migration 252 recorded in PROD schema_migration; media-cards prod reports "done: 0 card copies"; a listing page in Safari shows card photos of ~100-300 KB in Web Inspector.',
 'None. DEV already done (165 copies, one AVIF skipped).'),

('MEDIA-AVIF-01', 'PUBLIC', 'AVIF photos get derived copies', 'Low', 'Planned',
 'Photos uploaded as AVIF get web, card and thumb copies like every other photo.',
 'The image crate build cannot decode AVIF (puerto-rico-ferry.jpg on DEV). Enable a Rust AVIF decoder, or convert on upload.',
 'media-cards reports 0 skipped on DEV.', 'MEDIA-CARD-PROD-01'),

('ENG-HARDENING-02', 'HARDEN', 'Production hardening - owner decisions', 'High', 'Planned',
 'Make the application harder to lose or break.',
 'Owner decides each: (1) raise Neon PROD restore history from 6 h (history_retention_seconds 21600); (2) cargo audit and the 45 Dependabot alerts (4 critical, 21 high); (3) a CI gate that builds and tests the Rust workspace; (4) rate limits on public and portal routes; (5) DB statement timeouts; (6) a restore drill onto a scratch branch.',
 'Each item is done or declined by the owner and recorded here.', ''),

('TECH-FLIGHT-RECORDER-01', 'TECH', 'Flight Recorder console ported to Rust/Yew', 'High', 'Ready',
 '/portal/tech/flight-recorder/:instanceId shows a Forge/workflow run end to end - the owner''s five-view console - with no JavaScript.',
 'Packet: docs/agent/packets/TECH-FLIGHT-RECORDER-01.md (research done, build plan written). Portal route, pure-Rust adapter and layouts, Screen-trait screen, Tech menu entry.',
 'All five views render a real DEV Forge trace in Safari; unit tests for the classifiers and both layouts; no TypeScript added.', 'Storyboard cleanup, so there are runs to look at.'),

('STORYBOARD-CLEANUP-01', 'FORGE', 'Storyboard cleanup and Rust recon', 'High', 'Planned',
 'The storyboard reflects reality, so Forge builds the right stories.',
 'Owner runs the 14 storyboard deletes (SQL given 2026-09-28). Recon the ~50 stories flipped to Rust, with DeepSeek: flip status or delete each stale story.',
 'No stale Planned/In Progress stories for work already done or abandoned.', ''),

('PROJECTS-DOCS-02', 'PROJECTS', 'Projects Documents pane - final copy', 'Medium', 'Planned',
 'The Documents pane wording matches the owner''s copy.',
 'Record signed / Mark signed / Signed copy to come shipped 2026-09-28. Apply the owner''s copy text when it arrives.',
 'Owner approves the wording in Safari.', 'Owner sends copy.'),

('PROJECTS-TREE-01', 'PROJECTS', 'Projects left-tree redesign', 'Medium', 'Planned',
 'A clearer, denser Projects navigator.',
 'Design together with the owner; smaller fonts. Do not start without the owner.',
 'Owner signs off on the design in Safari.', ''),

('TXN-CONTRACTS-WORKFLOWS-01', 'TXN', 'Contracts vs Workflows - decide the model', 'Medium', 'Planned',
 'One clear relationship between contracts (forms) and workflows.', 'Owner to scope.', 'Owner-approved design.', ''),

('CRM-SELLER-STRATEGY-01', 'CRM', 'Seller Strategy', 'Medium', 'Planned',
 'Seller strategy screen and workflow.', 'Owner to scope.', 'Owner-approved design.', ''),

('ACCT-FINANCIALS-01', 'ACCOUNTING', 'Financials', 'Medium', 'Planned',
 'Financials in the Rust app.', 'Owner to scope; relates to ACCT-01 and ACCT-02.', 'Owner-approved design.', ''),

('SUPPORT-PASS-01', 'SUPPORT', 'Support pass', 'Medium', 'Planned',
 'Review and fix the support surface.', 'Owner to scope.', 'Owner sign-off.', ''),

('MARKETING-REDESIGN-01', 'MARKETING', 'Marketing redesign', 'Medium', 'Planned',
 'Redesigned marketing surface.', 'Owner to scope; intended for GPT.', 'Owner sign-off.', '')
on conflict (id) do nothing;

select id, status, priority, title from storyboard_story
 where id in ('MEDIA-CARD-PROD-01','MEDIA-AVIF-01','ENG-HARDENING-02','TECH-FLIGHT-RECORDER-01','STORYBOARD-CLEANUP-01',
              'PROJECTS-DOCS-02','PROJECTS-TREE-01','TXN-CONTRACTS-WORKFLOWS-01','CRM-SELLER-STRATEGY-01',
              'ACCT-FINANCIALS-01','SUPPORT-PASS-01','MARKETING-REDESIGN-01','ENG-FORGE-SECRET-HISTORY-01')
 order by id;

commit;
