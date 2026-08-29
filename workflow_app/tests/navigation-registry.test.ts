// ---------------------------------------------------------------------------
// UI-01 — Operating-surface navigation registry: focused unit proofs.
//
//   1. representative routes map to the correct operating surface
//   2. sub-routes inherit their parent surface
//   3. every known /portal route belongs to a surface
//   4. selecting NEXUS/OPS/TECH/SUPPORT produces its contextual navigation
//   5. surface homes are stable and belong to their surface
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  OPERATING_SURFACE_ORDER,
  OPERATING_SURFACES,
  navigationForSurface,
  surfaceForPathname,
  surfaceHome,
} from '../../lib/navigation'

test('UI-01: representative routes map to the correct operating surface', () => {
  // NEXUS — primary real-estate operating environment
  assert.equal(surfaceForPathname('/portal/dashboard'), 'NEXUS')
  assert.equal(surfaceForPathname('/portal/deals'), 'NEXUS')
  assert.equal(surfaceForPathname('/portal/clients'), 'NEXUS')
  assert.equal(surfaceForPathname('/portal/showings'), 'NEXUS')
  // OPS — office/business administration
  assert.equal(surfaceForPathname('/portal/needs-review'), 'OPS')
  assert.equal(surfaceForPathname('/portal/identity-quality'), 'OPS')
  assert.equal(surfaceForPathname('/portal/reporting'), 'OPS')
  assert.equal(surfaceForPathname('/portal/settings'), 'SUPPORT')
  // TECH — greenfield engineering / Story Board / Forge
  assert.equal(surfaceForPathname('/portal/storyboard'), 'TECH')
  assert.equal(surfaceForPathname('/portal/command-console'), 'TECH')
  assert.equal(surfaceForPathname('/portal/command-center'), 'TECH')
  // NEXUS — transaction-oriented workflow instances (the rule: workflow FOR an
  // active real-estate transaction → NEXUS).
  assert.equal(surfaceForPathname('/portal/workflows'), 'NEXUS')
  // SUPPORT — keep-the-lights-on technology operations
  assert.equal(surfaceForPathname('/portal/system-health'), 'SUPPORT')
})

test('UI-01: sub-routes inherit their parent surface', () => {
  assert.equal(surfaceForPathname('/portal/command-console/CRM-19'), 'TECH')
  assert.equal(surfaceForPathname('/portal/storyboard/CRM-19'), 'TECH')
  assert.equal(surfaceForPathname('/portal/deals/some-deal-id'), 'NEXUS')
})

test('UI-01: every known /portal route belongs to a surface', () => {
  const routes = [
    '/portal/accounting',
    '/portal/accounting/receivables',
    '/portal/accounting/expenses',
    '/portal/accounting/pnl',
    '/portal/accounting/receipt-scanner',
    '/portal/activity',
    '/portal/attention',
    '/portal/catch-up',
    '/portal/clients',
    '/portal/client-admin',
    '/portal/command-center',
    '/portal/command-console',
    '/portal/core/seller-strategy',
    '/portal/dashboard',
    '/portal/db-test',
    '/portal/deals',
    '/portal/forms',
    '/portal/identity-quality',
    '/portal/media-admin',
    '/portal/media-test',
    '/portal/needs-review',
    '/portal/property-admin',
    '/portal/property-media',
    '/portal/reporting',
    '/portal/settings',
    '/portal/showings',
    '/portal/storyboard',
    '/portal/system-health',
    '/portal/workflows',
  ]
  for (const route of routes) {
    const surface = surfaceForPathname(route)
    assert.ok(
      OPERATING_SURFACE_ORDER.includes(surface),
      `${route} maps to a known surface (got ${surface})`,
    )
  }
})

test('UI-01: selecting NEXUS/OPS/TECH/SUPPORT produces correct contextual navigation', () => {
  const expected: Record<string, string[]> = {
    NEXUS: [
      // CORE — the visible primary operating menu (label relabelled to CORE;
      // the NEXUS token stays stable). Per the 2026-08-25 CTO/Product Owner
      // decision, Workflows and Forms are visible CORE destinations (supersedes
      // the earlier "hidden from visible CORE navigation" note).
      'Cockpit',
      'Clients',
      'Catch-Up',
      'Contracts',
      'Cabinet',
      'Workflows',
      'Forms',
      'Seller Strategy',
    ],
    OPS: [
      'Issue Queue',
      'Needs Review',
      'Property Admin',
      'Media Audit',
      'Property Media',
      'Identity Quality',
      'Client Administration',
      'Reporting',
    ],
    ACCOUNTING: [
      'Dashboard',
      'Receivables',
      'Expenses',
      'P&L Statement',
      'Receipt Scanner',
    ],
    TECH: [
      'Tech Overview',
      'Command Center',
      'Story Board',
      'Command Console',
      'UI Lab',
      'DB Test',
      'Media Test',
      'Flight Recorder',
      'GROK',
    ],
    SUPPORT: ['System Health', 'Security'],
  }
  assert.deepEqual(
    OPERATING_SURFACE_ORDER.map((s) => OPERATING_SURFACES[s].label),
    ['CORE', 'ACCOUNTING', 'OPPS', 'SUPPORT', 'TECH'],
  )
  for (const surface of OPERATING_SURFACE_ORDER) {
    const labels = navigationForSurface(surface).map((item) => item.label)
    assert.deepEqual(labels, expected[surface], `${surface} contextual nav`)
    for (const item of navigationForSurface(surface)) {
      assert.ok(item.href.startsWith('/portal/'), `${item.href} is an existing route`)
    }
  }
})

test('UI-01: surface homes are stable and belong to their surface', () => {
  assert.deepEqual(
    OPERATING_SURFACE_ORDER.map((s) => surfaceHome(s)),
    [
      '/portal/dashboard',
      '/portal/accounting',
      '/portal/needs-review',
      '/portal/system-health',
      '/portal/tech',
    ],
  )
  for (const surface of OPERATING_SURFACE_ORDER) {
    const home = surfaceHome(surface)
    assert.equal(
      surfaceForPathname(home),
      surface,
      `${home} belongs to ${surface}`,
    )
  }
})
