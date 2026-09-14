import { publishAcceptedCandidate } from '../../lib/worker-workspace'
import { getStoryboardStory } from '../../db/storyboard'
import { parseAssayCommands } from '../../agent-runtime/assay-plan'
import { runAssayCommand } from '../../agent-runtime/deterministic-assay-adapter'
import {
  mergeForgeWorkflowEvidence,
  readForgeWorkflowEvidence,
} from '../../db/forge-workflow-evidence'
import {
  FORGE_MIGRATE_DEV,
  FORGE_MIGRATE_PROD,
  FORGE_PUBLISH_CANDIDATE,
  FORGE_REFRESH_DERIVED_MODELS,
  FORGE_VERIFY_DERIVED_MODELS,
  FORGE_VERIFY_DEV_MIGRATION,
  FORGE_VERIFY_PROD_MIGRATION,
} from '../forge-command-types'
import { forgeLineageError, type ForgeGateEvidence } from './forge-facts'
import {
  createForgeReleaseOperations,
  type ForgeOperationResult,
  type ForgeReleaseOperations,
  type ForgeReleaseTarget,
} from './release-operations'
import type {
  ForgeCommandEnvelope,
  ForgeCommandResult,
  ForgeReleaseExecutor,
} from './forge-state-writer'

function precondition(commandType: string, message: string): ForgeCommandResult {
  return { commandType, outcome: 'precondition_failure', message }
}

function requireContext(envelope: ForgeCommandEnvelope): {
  processInstanceId: string
  storyId: string
} | null {
  const processInstanceId = envelope.processInstanceId?.trim()
  const storyId = envelope.storyId?.trim()
  return processInstanceId && storyId ? { processInstanceId, storyId } : null
}

/**
 * Production release-command adapter. Implemented commands perform their real
 * side effect; commands not yet backed by a canonical operation fail closed
 * instead of returning synthetic success.
 */
export function createDbForgeReleaseExecutor(
  repoRoot = process.cwd(),
  deps: {
    readEvidence?: typeof readForgeWorkflowEvidence
    mergeEvidence?: typeof mergeForgeWorkflowEvidence
    publish?: typeof publishAcceptedCandidate
    operations?: ForgeReleaseOperations
    /**
     * The evidence of the task completing RIGHT NOW.
     *
     * Release commands execute INSIDE the transition that follows QA, and the runtime merges the
     * evidence row only AFTER that transition returns. So a publish command reading the row sees the
     * PRE-QA state: `qaVerifiedSha` is not there yet, `forgeLineageError(evidence,'qa')` refuses with
     * "qaVerifiedSha is missing or invalid", and publish is recorded as failed with no reason - while
     * the row that lands a moment later says `qa_passed = true` with `qa_verified_sha == candidate_sha`
     * (measured twice on 2026-09-14, instances 8a2c9b00 and 123de634).
     *
     * Merging the in-flight evidence over the row is the same rule the fact reader follows: the turn
     * being completed is the most recent truth available.
     */
    pendingEvidence?: ForgeGateEvidence
  } = {},
): ForgeReleaseExecutor {
  const readEvidence = deps.readEvidence ?? readForgeWorkflowEvidence
  const mergeEvidence = deps.mergeEvidence ?? mergeForgeWorkflowEvidence
  const publish = deps.publish ?? publishAcceptedCandidate
  const operations = deps.operations ?? createForgeReleaseOperations()
  return {
    async execute(envelope): Promise<ForgeCommandResult> {
      const context = requireContext(envelope)
      if (!context) {
        return precondition(envelope.commandType, 'Forge release command is missing process/story context')
      }

      const stored = await readEvidence(context.storyId)
      const evidence: ForgeGateEvidence = deps.pendingEvidence
        ? { ...stored, ...deps.pendingEvidence }
        : stored
      if (envelope.commandType !== FORGE_PUBLISH_CANDIDATE) {
        const migrationCommand = migrationCommandPlan(envelope.commandType)
        if (migrationCommand) {
          let result: ForgeOperationResult
          try {
            const input = {
              storyId: context.storyId,
              target: migrationCommand.target,
              migrationFiles: evidence.migrationFiles ?? [],
              repoRoot,
            }
            result = migrationCommand.verify
              ? await operations.verifyMigrations(input)
              : await operations.applyMigrations({ ...input, commandId: envelope.commandId })
          } catch (error) {
            result = { success: false, detail: String((error as Error)?.message ?? error) }
          }
          const failedReleaseStage =
            migrationCommand.target === 'dev' ? ('DEV_MIGRATION' as const) : ('PROD_MIGRATION' as const)
          await mergeEvidence(context.processInstanceId, context.storyId, {
            ...(migrationCommand.target === 'dev'
              ? migrationCommand.verify
                ? { devMigrationVerified: result.success }
                : { devMigrationApplied: result.success }
              : migrationCommand.verify
                ? { prodMigrationVerified: result.success }
                : { prodMigrationApplied: result.success }),
            ...(!result.success
              ? {
                  failureClass: 'MIGRATION' as const,
                  failedReleaseStage,
                }
              : {}),
          })
          return { commandType: envelope.commandType, outcome: 'success', message: result.detail }
        }

        if (
          envelope.commandType === FORGE_REFRESH_DERIVED_MODELS ||
          envelope.commandType === FORGE_VERIFY_DERIVED_MODELS
        ) {
          let result: ForgeOperationResult
          const verify = envelope.commandType === FORGE_VERIFY_DERIVED_MODELS
          try {
            const input = {
              storyId: context.storyId,
              target: 'prod' as const,
              models: evidence.derivedModels ?? [],
            }
            result = verify
              ? await operations.verifyDerived(input)
              : await operations.refreshDerived({ ...input, commandId: envelope.commandId })
          } catch (error) {
            result = { success: false, detail: String((error as Error)?.message ?? error) }
          }
          await mergeEvidence(context.processInstanceId, context.storyId, {
            ...(verify
              ? { derivedRefreshVerified: result.success }
              : { derivedRefreshSucceeded: result.success }),
            ...(!result.success
              ? {
                  failureClass: 'ENVIRONMENT' as const,
                  failedReleaseStage: 'DERIVED_REFRESH' as const,
                }
              : {}),
          })
          return { commandType: envelope.commandType, outcome: 'success', message: result.detail }
        }

        return precondition(envelope.commandType, `unsupported Forge release command ${envelope.commandType}`)
      }

      const lineageError = forgeLineageError(evidence, 'qa')
      if (lineageError) {
        await mergeEvidence(context.processInstanceId, context.storyId, {
          publishSucceeded: false,
          failureClass: 'PUBLISH_CONFLICT',
          failedReleaseStage: 'PUBLISH',
        })
        // The command itself executed truthfully; the following XML decision
        // owns repair routing from the persisted business result.
        return { commandType: envelope.commandType, outcome: 'success', message: lineageError }
      }

      // THE FROZEN PROOFS RE-VERIFY AN INTEGRATED CANDIDATE.
      //
      // Without this callback the publisher still refuses a moved main, so the integration path
      // would exist and never run. The proofs are the STORY's own frozen commands (read from the
      // story, parsed by the same parser the Lead and the Assay use — never model prose), and
      // they run in the integration worktree against the merged tree, so a merge that merely
      // applies cleanly cannot publish: it has to PASS.
      const proofStory = await getStoryboardStory(context.storyId).catch(() => null)
      const frozenProofs = parseAssayCommands(proofStory?.assayCommands ?? null)
      const result = await publish({
        repoRoot,
        candidateCommit: evidence.candidateSha,
        ...(frozenProofs.length > 0
          ? {
              verifyIntegrated: async ({ cwd }: { cwd: string; integratedCommit: string }) => {
                const failed: string[] = []
                for (const command of frozenProofs) {
                  const outcome = await runAssayCommand({
                    command,
                    cwd,
                    env: { ...process.env },
                    timeoutMs: 120_000,
                  }).catch(() => null)
                  if (!outcome || outcome.exitCode !== 0) {
                    failed.push(`${command} (exit ${outcome?.exitCode ?? 'unmeasurable'})`)
                  }
                }
                return failed.length > 0
                  ? { ok: false, detail: `frozen proofs failed on the integrated tree: ${failed.join(' | ')}` }
                  : { ok: true, detail: `frozen proofs passed on the integrated tree (${frozenProofs.length})` }
              },
            }
          : {}),
      })
      // BOTH published shapes are success. `integrated-and-published` means main had moved,
      // the candidate was merged onto the current head, the integrated tree was re-verified,
      // and that commit is now on main — a completion, not a conflict.
      if (result.outcome === 'published' || result.outcome === 'integrated-and-published') {
        await mergeEvidence(context.processInstanceId, context.storyId, {
          publishSucceeded: true,
          publishedSha: result.publishedMainHash,
        })
        return {
          commandType: envelope.commandType,
          outcome: 'success',
          message: `published ${result.publishedMainHash}`,
        }
      }

      // Every non-published outcome is reported with ITS OWN reason. `integration-unverified`
      // and `integration-conflict` are new: the first means the candidate WAS merged onto a
      // moved main and the integrated tree failed its proofs (so nothing was published), the
      // second means the merge itself conflicted. Naming which one happened is the difference
      // between "retry" and "a human must look".
      const message =
        result.outcome === 'no-candidate'
          ? result.reason
          : result.outcome === 'integration-unverified'
            ? `integration produced ${result.integratedCommit} but it did not verify: ${result.reason}`
            : result.outcome === 'integration-conflict'
              ? result.reason
              : result.reason /* publish-conflict */

      await mergeEvidence(context.processInstanceId, context.storyId, {
        publishSucceeded: false,
        failureClass: 'PUBLISH_CONFLICT',
        failedReleaseStage: 'PUBLISH',
        // THE REASON IS PART OF THE OUTCOME.
        //
        // This branch used to record that publish failed and nothing else, so the evidence row said
        // `publish_succeeded = false` with no reason anywhere and neither the operator nor the repair
        // classifier could learn that, say, a frozen proof could not run in the integration tree.
        // `lastFailure` carries the publisher's own words, so the reason outlives the turn.
        lastFailure: message,
      })

      return {
        commandType: envelope.commandType,
        outcome: 'success',
        message,
      }
    },
  }
}

function migrationCommandPlan(commandType: string): {
  target: ForgeReleaseTarget
  verify: boolean
} | null {
  switch (commandType) {
    case FORGE_MIGRATE_DEV:
      return { target: 'dev', verify: false }
    case FORGE_VERIFY_DEV_MIGRATION:
      return { target: 'dev', verify: true }
    case FORGE_MIGRATE_PROD:
      return { target: 'prod', verify: false }
    case FORGE_VERIFY_PROD_MIGRATION:
      return { target: 'prod', verify: true }
    default:
      return null
  }
}
