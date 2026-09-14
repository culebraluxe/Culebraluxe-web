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
// The commit comes from Vercel's build environment (server-rendered, so no
// client env plumbing and no secret). Locally it is honest about being local.
// ---------------------------------------------------------------------------

export const COCKPIT_VERSION = 'V2'

export function cockpitBuildLabel(): string {
  const sha =
    process.env.VERCEL_GIT_COMMIT_SHA ??
    process.env.NEXT_PUBLIC_VERCEL_GIT_COMMIT_SHA ??
    process.env.GIT_COMMIT_SHA ??
    ''
  const short = sha.trim().slice(0, 7)
  if (short) return short
  // Not deployed (a developer's machine, or a preview without git metadata).
  return process.env.NODE_ENV === 'production' ? 'sha unknown' : 'local'
}

/** What the corner shows: `V2 · cf21241`. */
export function cockpitVersionLabel(): string {
  return `${COCKPIT_VERSION} · ${cockpitBuildLabel()}`
}
