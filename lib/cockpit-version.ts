// ---------------------------------------------------------------------------
// COCKPIT VERSION MARKER — so a screenshot, a phone call or a bug report can say
// WHICH COCKPIT is on screen.
//
// The captain closed and reopened the browser twice trying to escape a stale
// bundle and could not tell whether he had succeeded, because nothing on the page
// named the build. That is a real gap: when the answer to "are you on the new
// version?" is "look at the corner", nobody has to guess, and nobody has to be
// paranoid about their cache.
//
// WHERE THE SHA COMES FROM: production is built LOCALLY (`bash scripts/vercel-build-prod.sh`)
// and deployed prebuilt (`vercel deploy --prebuilt --prod`), so Vercel's own git
// variables may not be set at all. The build script therefore stamps the commit into
// `NEXT_PUBLIC_COCKPIT_SHA`, which is inlined into the bundle at build time - the
// artifact names its own source rather than asking the platform what it thinks it is.
// Vercel's variables remain as fallbacks for a platform build, and a local build says
// so honestly instead of inventing a sha.
// ---------------------------------------------------------------------------

export const COCKPIT_VERSION = 'V2'

function cleaned(value: string | undefined): string | null {
  const text = (value ?? '').trim()
  return text.length > 0 ? text : null
}

export function cockpitBuildLabel(): string {
  const sha =
    cleaned(process.env.NEXT_PUBLIC_COCKPIT_SHA) ??
    cleaned(process.env.VERCEL_GIT_COMMIT_SHA) ??
    cleaned(process.env.NEXT_PUBLIC_VERCEL_GIT_COMMIT_SHA) ??
    cleaned(process.env.GIT_COMMIT_SHA)
  if (sha) return sha.slice(0, 7)
  // Not a stamped production artifact (a developer's machine, or a build that never went through the
  // release script). Say that plainly - `sha unknown` in production is a real finding, not a cosmetic one.
  return process.env.NODE_ENV === 'production' ? 'sha unknown' : 'local'
}

/** When this artifact was built, from the same stamp. Empty string when unstamped. */
export function cockpitBuiltAt(): string {
  return cleaned(process.env.NEXT_PUBLIC_COCKPIT_BUILT_AT) ?? ''
}

/** What the corner shows: `V2 · f61ebbc`. */
export function cockpitVersionLabel(): string {
  return `${COCKPIT_VERSION} · ${cockpitBuildLabel()}`
}
