// ---------------------------------------------------------------------------
// smoke:prod — ask production whether it is actually working, not just deployed.
//
//   pnpm smoke:prod                    # health: does the site answer, and with the right shape
//   pnpm smoke:prod --expect-head      # also assert the live sha equals HEAD (the deploy path)
//   pnpm smoke:prod --format json
//
// Pirated from OpenContext's `.github/workflows/cli-smoke.yml` (0xranx/OpenContext, MIT),
// which installs the package it just built and runs it, because a test suite answers "does
// the code work" and only the deployed artefact answers "does the thing people load work".
// Our deploy script verified the sha from /api/build-info and stopped there: a deploy could
// alias to a build whose pages 500 and still report VERIFIED.
//
// Two deliberate choices:
//   - the CANONICAL domain, never a *.vercel.app URL (Vercel Authentication answers 302 to a
//     login page there, measured 2026-09-14, so a check against it can never pass);
//   - checks are written against strings the pages own (title, "Selected Properties"), not
//     byte counts, so a copy edit does not turn a smoke test into a false alarm.
// ---------------------------------------------------------------------------

import { execFileSync } from 'node:child_process'

const DEFAULT_URL = 'https://www.culebraluxe.com'
const TIMEOUT_MS = Number(process.env.SMOKE_TIMEOUT_MS ?? 25_000)

type Check = { name: string; url: string; ok: boolean; detail: string }

function gitHead(): { short: string; subject: string } | null {
  try {
    const short = execFileSync('git', ['rev-parse', '--short', 'HEAD'], { encoding: 'utf8' }).trim()
    const subject = execFileSync('git', ['log', '-1', '--pretty=%s'], { encoding: 'utf8' }).trim()
    return { short, subject }
  } catch {
    return null
  }
}

/** How far behind HEAD the live build is, or null when the sha is not in this checkout. */
export function commitsBehind(liveSha: string, head: string): number | null {
  try {
    const out = execFileSync('git', ['rev-list', '--count', `${liveSha}..${head}`], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim()
    const value = Number.parseInt(out, 10)
    return Number.isFinite(value) ? value : null
  } catch {
    return null
  }
}

async function fetchText(url: string): Promise<{ status: number; body: string }> {
  const response = await fetch(url, {
    redirect: 'follow',
    signal: AbortSignal.timeout(TIMEOUT_MS),
    headers: { 'user-agent': 'culebraluxe-prod-smoke' },
  })
  return { status: response.status, body: await response.text() }
}

async function main(): Promise<number> {
  const argv = process.argv.slice(2)
  const json = argv.includes('--format') && argv[argv.indexOf('--format') + 1] === 'json'
  const expectHead = argv.includes('--expect-head')
  const expectShaIndex = argv.indexOf('--expect-sha')
  const expectedSha = expectShaIndex >= 0 ? (argv[expectShaIndex + 1] ?? '') : ''
  const base = (process.env.CULEBRALUXE_PROD_URL ?? DEFAULT_URL).replace(/\/+$/, '')

  const head = gitHead()
  const checks: Check[] = []

  // 1. The build stamp: what is live, and is it a build we recognise.
  let liveSha = ''
  let liveVersion = ''
  let liveBuiltAt = ''
  try {
    const { status, body } = await fetchText(`${base}/api/build-info`)
    const parsed = JSON.parse(body) as { version?: string; sha?: string; builtAt?: string }
    liveSha = parsed.sha ?? ''
    liveVersion = parsed.version ?? ''
    liveBuiltAt = parsed.builtAt ?? ''
    const shaped = status === 200 && /^[0-9a-f]{7,40}$/.test(liveSha) && liveVersion.length > 0
    checks.push({
      name: 'build-info is a real build stamp',
      url: `${base}/api/build-info`,
      ok: shaped,
      detail: shaped
        ? `${liveVersion} · ${liveSha} · built ${liveBuiltAt}`
        : `status ${status}, unexpected body: ${body.slice(0, 120)}`,
    })
  } catch (error) {
    checks.push({
      name: 'build-info is a real build stamp',
      url: `${base}/api/build-info`,
      ok: false,
      detail: error instanceof Error ? error.message : String(error),
    })
  }

  // 2. The public homepage is server-rendered enough that its brand marker is present in the response.
  try {
    const { status, body } = await fetchText(`${base}/`)
    const hasMarker = body.includes('CulebraLuxe')
    checks.push({
      name: 'home page renders',
      url: `${base}/`,
      ok: status === 200 && hasMarker,
      detail:
        status !== 200
          ? `status ${status}`
          : hasMarker
            ? `200, ${Math.round(body.length / 1024)}KB, marker "CulebraLuxe" present`
            : '200 but the CulebraLuxe marker is missing',
    })
  } catch (error) {
    checks.push({
      name: 'home page renders',
      url: `${base}/`,
      ok: false,
      detail: error instanceof Error ? error.message : String(error),
    })
  }

  // 3. /buyers is Yew-owned and therefore CLIENT rendered. Its HTTP document intentionally contains only
  // the Rust mount point; "Selected Properties" is emitted by WASM after boot and can never be a reliable
  // curl/fetch smoke marker. Prove both halves instead:
  //   a) the deployed page contains the Rust/Yew mount;
  //   b) the production page-data endpoint returns at least one public listing.
  try {
    const { status, body } = await fetchText(`${base}/buyers`)
    const hasMount = body.includes('id="rust-ui"')
    checks.push({
      name: 'buyers Yew shell renders',
      url: `${base}/buyers`,
      ok: status === 200 && hasMount,
      detail:
        status !== 200
          ? `status ${status}`
          : hasMount
            ? '200, Rust/Yew mount present'
            : '200 but the #rust-ui mount is missing',
    })
  } catch (error) {
    checks.push({
      name: 'buyers Yew shell renders',
      url: `${base}/buyers`,
      ok: false,
      detail: error instanceof Error ? error.message : String(error),
    })
  }

  try {
    const url = `${base}/api/rust-ui/public-page?screen=site-buyers`
    const { status, body } = await fetchText(url)
    const parsed = JSON.parse(body) as { listings?: unknown[] }
    const count = Array.isArray(parsed.listings) ? parsed.listings.length : 0
    checks.push({
      name: 'buyers production inventory loads',
      url,
      ok: status === 200 && count > 0,
      detail:
        status !== 200
          ? `status ${status}`
          : count > 0
            ? `200, ${count} public listing(s)`
            : '200 but production returned zero public listings',
    })
  } catch (error) {
    checks.push({
      name: 'buyers production inventory loads',
      url: `${base}/api/rust-ui/public-page?screen=site-buyers`,
      ok: false,
      detail: error instanceof Error ? error.message : String(error),
    })
  }

  // 4. The Rust cutover proof. This goes through the public frontend, which then calls the
  // private service binding. A 200 therefore proves the frontend has RUST_API_BASE_URL, the Rust
  // container is running, and that container can ping the database it selected. Production must
  // report "prod" — a live container pointed at DEV is a failed release, not a partial success.
  try {
    const { status, body } = await fetchText(`${base}/api/rust-ready`)
    const parsed = JSON.parse(body) as { ok?: boolean; databaseTarget?: string | null; error?: string }
    const ready = status === 200 && parsed.ok === true && parsed.databaseTarget === 'prod'
    checks.push({
      name: 'Rust API is ready on PROD database',
      url: `${base}/api/rust-ready`,
      ok: ready,
      detail: ready
        ? 'frontend binding -> Rust container -> PROD Neon'
        : `status ${status}, target ${parsed.databaseTarget ?? '<none>'}, ${parsed.error ?? 'not ready'}`,
    })
  } catch (error) {
    checks.push({
      name: 'Rust API is ready on PROD database',
      url: `${base}/api/rust-ready`,
      ok: false,
      detail: error instanceof Error ? error.message : String(error),
    })
  }

  // 5. The sha assertion, only when the caller asks for it (the deploy path does).
  const wanted = expectedSha || (expectHead && head ? head.short : '')
  if (wanted) {
    // THE TWO SIDES ARE NOT THE SAME WIDTH, BY DESIGN. `/api/build-info` serves `cockpitBuildLabel()`, the
    // first SEVEN characters of the stamped commit, while `--expect-head` supplies `git rev-parse --short`,
    // which lengthens as the repository grows (it returned 8 here on 2026-09-16). Comparing the strings whole
    // asked "did git's abbreviation grow", not "is the live build HEAD". Compare the length they SHARE.
    const shared = Math.min(liveSha.length, wanted.length)
    const agrees = liveSha !== '' && shared > 0 && liveSha.slice(0, shared) === wanted.slice(0, shared)
    checks.push({
      name: `live sha equals ${wanted}`,
      url: `${base}/api/build-info`,
      ok: agrees,
      detail: agrees ? `serving ${liveSha}` : `live ${liveSha || '<no answer>'} vs expected ${wanted}`,
    })
  }

  const behind = liveSha && head ? commitsBehind(liveSha, head.short) : null
  const failures = checks.filter((check) => !check.ok)

  if (json) {
    console.log(
      JSON.stringify(
        {
          base,
          live: { sha: liveSha, version: liveVersion, builtAt: liveBuiltAt },
          head: head?.short ?? null,
          commitsBehind: behind,
          failures: failures.length,
          checks,
        },
        null,
        2,
      ),
    )
    return failures.length > 0 ? 1 : 0
  }

  console.log(`Production smoke — ${base}`)
  for (const check of checks) {
    console.log(`  ${check.ok ? 'ok  ' : 'FAIL'}  ${check.name}`)
    console.log(`        ${check.detail}`)
  }
  if (behind !== null && behind > 0) {
    // Reported, never failed: a release per sprint means production is SUPPOSED to sit
    // behind HEAD between releases. Saying so is the point; failing would be wrong.
    console.log(
      `\n  note: production is ${behind} commit(s) behind HEAD (${head?.short}) — expected between ` +
        'releases, and the Cockpit corner shows which build is live.',
    )
  }
  console.log(
    `\nsmoke:prod — ${checks.length - failures.length}/${checks.length} checks passed` +
      `${liveSha ? `, live ${liveVersion} ${liveSha}` : ', live sha unknown'}`,
  )
  return failures.length > 0 ? 1 : 0
}

void main().then((code) => process.exit(code))
