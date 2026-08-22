-- CulebraLuxe Portal
-- SB-01 — Operating Surface Classification + Rollups
-- Migration: 053_storyboard_sb01_operating_surface.sql
--
-- DELIBERATE classification of every stored Story Board story onto the
-- authoritative four-surface operating taxonomy (NEXUS | OPS | SUPPORT | TECH),
-- using the UI-01 screen mapping as the first precedent and classifying by the
-- PRIMARY JOB the story exists to support:
--
--   NEXUS    = do the real-estate work
--   OPS      = administer the business
--   SUPPORT  = operate, secure, diagnose and recover the built product
--   TECH     = build or change the product/platform
--
-- Only operating_surface is written (plus updated_at bookkeeping, following the
-- repo migration convention). status / completion / weight / dependencies /
-- workstream / execution state / queue state / run evidence are UNTOUCHED.
--
-- Deterministic per-id mapping (114 rows) — the VALUES list IS the record.
--
-- Recorded judgments (ambiguous seams):
--   * ARCH-HANDOFF  -> NULL   : deliberate non-executable reference/continuity
--                               record ("ultimate README"). No operating job —
--                               kept NULL per the reference-row rule.
--   * SOP1          -> SUPPORT: deliberate non-executable reference SOP whose
--                               entire purpose is operating the Forge
--                               (keep-the-lights-on) — SUPPORT is compelling.
--   * PORTAL-01     -> TECH   : parent umbrella for Portal Entry + Auth; its
--                               children AUTH-01..05 are mechanism/security
--                               engineering, so the parent is the access
--                               mechanism build (TECH), not NEXUS.
--   * ENG-20-SMOKE-001 / ENG-20B-PROOF-001 -> TECH : engineering-evidence
--                               fixtures (evidence register) — TECH is compelling.
--   * CRM-14H       -> SUPPORT: workflow operational seams (observability,
--                               recovery, admin controls).
--   * CRM-14A/C/D/E -> NEXUS  : transaction-workflow DOMAIN semantics (closing
--                               business); engine engineering stays TECH (ENG-xx).
--   * AUTH-00B      -> SUPPORT: security administration UI = security ops,
--                               not business user administration (OPS).
--
-- Idempotent: the distinct guard makes already-correct rows a no-op.

begin;

with classification (id, surface) as (
  values
    -- NEXUS — do the real-estate work (47)
    ('CRM-07', 'NEXUS'),
    ('CRM-08', 'NEXUS'),
    ('CRM-09A', 'NEXUS'),
    ('CRM-09C', 'NEXUS'),
    ('CRM-09D', 'NEXUS'),
    ('CRM-10', 'NEXUS'),
    ('CRM-11', 'NEXUS'),
    ('CRM-12A', 'NEXUS'),
    ('CRM-12B', 'NEXUS'),
    ('CRM-13', 'NEXUS'),
    ('CRM-14', 'NEXUS'),
    ('CRM-14A', 'NEXUS'),
    ('CRM-14C', 'NEXUS'),
    ('CRM-14D', 'NEXUS'),
    ('CRM-14E', 'NEXUS'),
    ('CRM-15', 'NEXUS'),
    ('CRM-16', 'NEXUS'),
    ('CRM-19', 'NEXUS'),
    ('CRM-20', 'NEXUS'),
    ('CRM-21', 'NEXUS'),
    ('CRM-22', 'NEXUS'),
    ('CRM-23', 'NEXUS'),
    ('CRM-25', 'NEXUS'),
    ('DOC-01', 'NEXUS'),
    ('DOC-02', 'NEXUS'),
    ('DOC-05', 'NEXUS'),
    ('INTAKE-01', 'NEXUS'),
    ('INTAKE-02', 'NEXUS'),
    ('POLISH-01', 'NEXUS'),
    ('POLISH-02', 'NEXUS'),
    ('POLISH-03', 'NEXUS'),
    ('PORTAL-03', 'NEXUS'),
    ('PORTAL-04', 'NEXUS'),
    ('PX-12', 'NEXUS'),
    ('PX-13', 'NEXUS'),
    ('PX-14', 'NEXUS'),
    ('PX-15', 'NEXUS'),
    ('PX-17', 'NEXUS'),
    ('PX-19', 'NEXUS'),
    ('PX-20', 'NEXUS'),
    ('PX-21', 'NEXUS'),
    ('PX-22', 'NEXUS'),
    ('PX-23', 'NEXUS'),
    ('PX-24', 'NEXUS'),
    ('PX-26', 'NEXUS'),
    ('PX-27', 'NEXUS'),
    ('PX-28', 'NEXUS'),
    -- OPS — administer the business (12)
    ('CRM-09B', 'OPS'),
    ('CRM-17', 'OPS'),
    ('CRM-18', 'OPS'),
    ('CRM-24', 'OPS'),
    ('OPS-02', 'OPS'),
    ('OPS-03', 'OPS'),
    ('OPS-04', 'OPS'),
    ('OPS-05', 'OPS'),
    ('OPS-06', 'OPS'),
    ('OPS-11', 'OPS'),
    ('PLAT-01', 'OPS'),
    ('PX-25', 'OPS'),
    -- SUPPORT — operate / secure / diagnose / recover (8)
    ('AUTH-00B', 'SUPPORT'),
    ('AUTH-04', 'SUPPORT'),
    ('AUTH-05', 'SUPPORT'),
    ('CRM-14H', 'SUPPORT'),
    ('OPS-01', 'SUPPORT'),
    ('OPS-09', 'SUPPORT'),
    ('OPS-10', 'SUPPORT'),
    ('SOP1', 'SUPPORT'),
    -- TECH — build or change the product/platform (46)
    ('AUTH-00A', 'TECH'),
    ('AUTH-01', 'TECH'),
    ('AUTH-02', 'TECH'),
    ('AUTH-03', 'TECH'),
    ('AUTH-06', 'TECH'),
    ('CRM-14B', 'TECH'),
    ('CRM-14F', 'TECH'),
    ('CRM-14G', 'TECH'),
    ('CRM-14I', 'TECH'),
    ('CRM-14J', 'TECH'),
    ('DOC-03', 'TECH'),
    ('DOC-04', 'TECH'),
    ('ENG-01', 'TECH'),
    ('ENG-02', 'TECH'),
    ('ENG-03', 'TECH'),
    ('ENG-04', 'TECH'),
    ('ENG-05', 'TECH'),
    ('ENG-06', 'TECH'),
    ('ENG-07', 'TECH'),
    ('ENG-08', 'TECH'),
    ('ENG-09', 'TECH'),
    ('ENG-10', 'TECH'),
    ('ENG-11', 'TECH'),
    ('ENG-12', 'TECH'),
    ('ENG-13', 'TECH'),
    ('ENG-14', 'TECH'),
    ('ENG-15', 'TECH'),
    ('ENG-16', 'TECH'),
    ('ENG-17', 'TECH'),
    ('ENG-18', 'TECH'),
    ('ENG-19', 'TECH'),
    ('ENG-20', 'TECH'),
    ('ENG-20-SMOKE-001', 'TECH'),
    ('ENG-20B', 'TECH'),
    ('ENG-20B-PROOF-001', 'TECH'),
    ('ENG-21', 'TECH'),
    ('ENG-22', 'TECH'),
    ('ENG-23', 'TECH'),
    ('ENG-24', 'TECH'),
    ('ENG-25', 'TECH'),
    ('ENG-26', 'TECH'),
    ('ENG-27', 'TECH'),
    ('OPS-07', 'TECH'),
    ('OPS-08', 'TECH'),
    ('PORTAL-01', 'TECH'),
    ('PORTAL-02', 'TECH'),
    -- Deliberate NULL — reference/continuity record (1)
    ('ARCH-HANDOFF', NULL)
)
update storyboard_story s
set operating_surface = c.surface,
    updated_at = now()
from classification c
where s.id = c.id
  and s.operating_surface is distinct from c.surface;

commit;
