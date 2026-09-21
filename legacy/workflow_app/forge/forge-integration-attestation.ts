// ---------------------------------------------------------------------------
// TECH-DEBT-07, DIALED BACK — the release attestation a release engineer can
// actually make.
//
// The shipped gate (forge-role-mapping.ts, `deploy`) demands
// `releaseEvidence.kind === 'deployment'`: a provider, a deployment id and a
// successful deploy state. Nothing in this repository can observe that — Vercel's
// deploy state is a HUMAN domain unless a reliable integration answers for it —
// so the requirement was unsatisfiable by design and every release-bearing story
// could only block (or be tempted to fabricate).
//
// The captain, 2026-09-12: "its asking DEVOPS too much — it should just say code
// is integrated and we got a clean build like a release engineer would and we have
// GIT number."
//
// So this is that attestation, and it refuses to claim anything it cannot observe:
//
//   * code integrated  — the exact artifact sha is contained in the declared
//                        integration ref (verified, not asserted);
//   * clean build      — a build command ACTUALLY RUN and exited 0, with its
//                        command, exit code and duration recorded;
//   * frozen proofs    — the story's frozen proofs ACTUALLY RAN and exited 0,
//                        recorded the same way, one observation per command;
//   * git number       — the sha itself, which is the receipt's identity.
//
// No deployment. No provider. No pretending. If the integration, a clean build or
// the frozen proofs are missing the attestation is not produced and the deploy gate
// stays shut, exactly as before.
//
// Pure except for the injected `isAncestor` probe, so it unit-tests without git.
// ---------------------------------------------------------------------------

import { execFileSync } from 'node:child_process'
import { gitBinary } from '@/lib/worker-workspace/provisioner'

import type { ReleaseEvidence } from '@/legacy/workflow_app/forge/forge-release-receipt'

export type BuildObservation = {
  command: string
  exitCode: number
  durationMs: number
}

export type IntegrationAttestation = {
  /** The sha proved to be contained in the integration ref. */
  artifactSha: string | null
  integratedRef: string
  integrated: boolean
  build: BuildObservation | null
  /**
   * The story's frozen proofs as they ACTUALLY RAN — one observation per command,
   * carrying its command, exit code and duration. Empty means nothing ran, so no
   * proof claim is made and the receipt is refused.
   */
  proofs: BuildObservation[] | null
  /** Why the attestation could not be made, when it could not. */
  reason: string | null
}

export type AttestIntegrationInput = {
  candidateSha: string | null | undefined
  /** The ref the code must be integrated INTO, taken from git facts. */
  integratedRef: string
  /** Containment probe, injected so this stays pure and testable. */
  isAncestor: (sha: string, ref: string) => boolean
  /** A build that actually ran. Null/absent means no clean-build claim. */
  build?: BuildObservation | null
  /** The frozen proofs that actually ran. Null/empty means no proof claim. */
  proofs?: BuildObservation[] | null
}

const SHA = /^[0-9a-f]{7,40}$/i

export function attestIntegration(input: AttestIntegrationInput): IntegrationAttestation {
  const base = {
    integratedRef: input.integratedRef,
    build: input.build ?? null,
    proofs: input.proofs ?? null,
  }

  const sha = (input.candidateSha ?? '').trim()
  if (!SHA.test(sha)) {
    return {
      ...base,
      artifactSha: null,
      integrated: false,
      reason: `no artifact sha to attest (got ${JSON.stringify(input.candidateSha ?? null)})`,
    }
  }

  let integrated = false
  try {
    integrated = input.isAncestor(sha, input.integratedRef)
  } catch (error) {
    return {
      ...base,
      artifactSha: sha,
      integrated: false,
      reason: `could not verify containment of ${sha} in ${input.integratedRef}: ${
        (error as Error)?.message ?? String(error)
      }`,
    }
  }
  if (!integrated) {
    return {
      ...base,
      artifactSha: sha,
      integrated: false,
      reason: `${sha} is not contained in ${input.integratedRef}`,
    }
  }

  const build = input.build ?? null
  const proofs = input.proofs ?? null
  // A COMMAND THAT NEVER RAN IS NOT A CLAIM. With neither a build nor a proof
  // observation there is nothing to attest, and the receipt is refused.
  if (!build && (!proofs || proofs.length === 0)) {
    return {
      ...base,
      artifactSha: sha,
      integrated: true,
      reason: 'no build was observed, so no clean-build claim is made',
    }
  }
  if (build && build.exitCode !== 0) {
    return {
      ...base,
      artifactSha: sha,
      integrated: true,
      reason: `build exited ${build.exitCode} (${build.command})`,
    }
  }
  const failedProof = (proofs ?? []).find((proof) => proof.exitCode !== 0)
  if (failedProof) {
    return {
      ...base,
      artifactSha: sha,
      integrated: true,
      reason: `frozen proof exited ${failedProof.exitCode} (${failedProof.command})`,
    }
  }

  return { ...base, artifactSha: sha, integrated: true, reason: null }
}

/**
 * Turn an attestation into release evidence — or null, which keeps the gate shut.
 *
 * The receiptId is prefixed `integration:` deliberately: the deploy gate can then
 * tell a release attestation apart from a deployment receipt, and nobody can
 * mistake one for the other on a board or in an audit.
 */
export function releaseEvidenceFromIntegration(
  attestation: IntegrationAttestation,
): ReleaseEvidence | null {
  if (!attestation.integrated || !attestation.artifactSha) return null
  const observed = [
    ...(attestation.build ? [attestation.build] : []),
    ...(attestation.proofs ?? []),
  ]
  if (observed.length === 0) return null
  if (observed.some((command) => command.exitCode !== 0)) return null
  return {
    kind: 'integration',
    artifactSha: attestation.artifactSha,
    receiptId: `integration:${attestation.integratedRef}@${attestation.artifactSha.slice(0, 12)}`,
    success: true,
  }
}

/** Git containment probe: is `sha` an ancestor of `ref` in this working tree? */
export function gitIsAncestor(cwd: string, sha: string, ref: string): boolean {
  try {
    execFileSync(gitBinary(), ['merge-base', '--is-ancestor', sha, ref], {
      cwd,
      stdio: 'ignore',
    })
    return true
  } catch {
    return false
  }
}
