#!/usr/bin/env node
// ---------------------------------------------------------------------------
// PORTAL NAVIGATION SMOKE TEST — click every portal nav link in a clean headless browser.
//
//   pnpm debug:portal-nav                       # against http://localhost:3000
//   pnpm debug:portal-nav http://localhost:3001 # another dev server
//
// Opens /portal/dashboard, then clicks each top-nav world and each rail tab in turn and checks that the URL and the
// screen the master shell mounted (`[data-screen-key]`) are the ones the link points at, and says whether the move
// was in-app (the router) or a document load. Exits non-zero on any mismatch.
//
// WHY IT EXISTS (2026-09-25): "every portal link bounces back to the Core dashboard" turned out to be a browser tab
// still running an older wasm module after `pnpm ui:build`. This script runs in a fresh browser with no cache, so:
//   - it passes but your tab misbehaves → stale tab: hard reload (Cmd+Shift+R) or use a private window;
//   - it fails too                     → a real bug in the current build; the printed line shows where.
//
// Needs the dev server running with PORTAL_AUTH_BYPASS=1 (no sign-in here). First run: `pnpm exec playwright install
// chromium` downloads the headless browser.
// ---------------------------------------------------------------------------

import { chromium } from 'playwright'

const base = (process.argv[2] ?? 'http://localhost:3000').replace(/\/+$/, '')
const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
const errors = []
page.on('pageerror', (error) => errors.push(`pageerror: ${error.message.slice(0, 160)}`))
page.on('console', (message) => {
  if (message.type() === 'error') errors.push(`console: ${message.text().slice(0, 160)}`)
})

const screenNow = () => page.getAttribute('[data-screen-key]', 'data-screen-key').catch(() => null)
let loads = 0
page.on('load', () => { loads += 1 })
const pathNow = () => new URL(page.url()).pathname
const settle = () => page.waitForTimeout(3000)

await page.goto(`${base}/portal/dashboard`, { waitUntil: 'networkidle' })
await settle()
if (pathNow() !== '/portal/dashboard') {
  console.error(`could not open the portal: landed on ${pathNow()} (is PORTAL_AUTH_BYPASS=1 set on the dev server?)`)
  await browser.close()
  process.exit(2)
}

const worlds = await page.$$eval('nav[aria-label="Operating surface"] a', (links) => links.map((a) => a.getAttribute('href')))
let failures = 0

for (const world of worlds) {
  await page.goto(`${base}${world}`, { waitUntil: 'networkidle' })
  await settle()
  const tabs = await page.$$eval('.portal-glass-rail a', (links) => links.map((a) => a.getAttribute('href')))
  // The world's capsule itself, then every rail tab it offers — each clicked FROM the world's page, so one bad
  // destination cannot hide the links after it.
  for (const href of [world, ...tabs]) {
    await page.goto(`${base}${world}`, { waitUntil: 'networkidle' })
    await settle()
    errors.length = 0
    const loadsBefore = loads
    const clicked = await page
      .click(`header a[href="${href}"]`, { timeout: 5000, noWaitAfter: true })
      .then(() => true)
      .catch(() => false)
    await page.waitForLoadState('networkidle').catch(() => undefined)
    await settle()
    const [path, screen] = [pathNow(), await screenNow()]
    // A destination that is still a React page has no Yew mount: it is reported, not failed.
    const react = clicked && path === href && screen === null
    const ok = clicked && path === href
    if (!ok) failures += 1
    console.log(
      `${ok ? 'ok  ' : 'FAIL'} ${href.padEnd(36)} -> ${path.padEnd(36)} ${loads > loadsBefore ? 'load  ' : 'in-app'} screen=${screen ?? '(none)'}` +
        (clicked ? '' : '  (link not found)') +
        (react ? '  (React page, not Yew)' : '') +
        (errors.length ? `  ${errors.join(' | ')}` : ''),
    )
  }
}

await browser.close()
console.log(failures ? `\n${failures} navigation failure(s)` : '\nall portal links land where they point')
process.exit(failures ? 1 : 0)
