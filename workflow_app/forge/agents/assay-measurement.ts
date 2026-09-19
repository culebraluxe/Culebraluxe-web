import { execFileSync } from 'node:child_process'
import { gitBinary } from '../../../lib/worker-workspace/provisioner'

/**
 * THE MEASUREMENT IDENTITY FOR A DIRECT-TO-QA ROUTE (work package A, item 7).
 *
 * The defect this closes is a labelling one, and it is the kind that looks like success: a route names the
 * commit it verifies, the proofs run somewhere else, and the receipt is stamped with the named sha. The
 * assertion passed — against code the route never identified — and the record claims otherwise.
 *
 * The operating model has no worktree to check out into ("NO TREE, SO NOTHING TO PIN": the QA lane runs the
 * frozen proofs in the primary checkout, `repoDir`). So there is exactly ONE honest thing this can say, and it
 * says it in two parts that must both be true:
 *
 *   * the tree the proofs actually ran against (its HEAD), recorded verbatim; and
 *   * that this tree CONTAINS the identified candidate, which is the only condition under which the pass is
 *     evidence about the identified work.
 *
 * If the tree does not contain the candidate, the route refuses — a concrete refusal naming both shas. It does
 * NOT fall back to "close enough", and it does not verify a tree the route never named.
 */

export type AssayMeasurement =
  | { ok: true; measuredSha: string; candidateSha: string; containsCandidate: true }
  | { ok: false; refusal: string }

type GitReader = (args: string[]) => string | null

/** The default reader: the same git abstraction the other repository probes use. */
export function gitReader(repoDir: string): GitReader {
  return (args) => {
    try {
      return execFileSync(gitBinary(), ['-C', repoDir, ...args], { stdio: 'pipe' }).toString().trim()
    } catch {
      return null
    }
  }
}

export function measureAssayCandidate(input: {
  repoDir: string
  candidateSha: string
  read?: GitReader
}): AssayMeasurement {
  const read = input.read ?? gitReader(input.repoDir)
  const candidateSha = input.candidateSha.trim().toLowerCase()
  if (!/^[0-9a-f]{40}$/.test(candidateSha)) {
    return { ok: false, refusal: `the identified candidate ${JSON.stringify(input.candidateSha)} is not a 40-hex sha` }
  }

  const measuredSha = read(['rev-parse', 'HEAD'])?.toLowerCase() ?? null
  if (!measuredSha) {
    return {
      ok: false,
      refusal: `the tree to measure could not be read (git rev-parse HEAD failed in ${input.repoDir}); nothing was verified`,
    }
  }

  // A tree that IS the candidate trivially contains it — and this is the case worth handling explicitly,
  // because `merge-base --is-ancestor X X` is true but the reader is the thing being trusted.
  if (measuredSha === candidateSha) {
    return { ok: true, measuredSha, candidateSha, containsCandidate: true }
  }

  const contains = read(['merge-base', '--is-ancestor', candidateSha, measuredSha])
  if (contains === null) {
    return {
      ok: false,
      refusal:
        `the identified candidate ${candidateSha} is not an ancestor of the tree the proofs would run against ` +
        `(${measuredSha}); verifying this tree would not be evidence about that candidate`,
    }
  }

  return { ok: true, measuredSha, candidateSha, containsCandidate: true }
}
