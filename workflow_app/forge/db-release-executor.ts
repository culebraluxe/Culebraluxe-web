import { publishAcceptedCandidate } from '../../lib/worker-workspace'
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
import { forgeLineageError } from './forge-facts'
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

      const evidence = await readEvidence(context.storyId)
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

      const result = await publish({
        repoRoot,
        candidateCommit: evidence.candidateSha,
      })
      if (result.outcome === 'published') {
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

      await mergeEvidence(context.processInstanceId, context.storyId, {
        publishSucceeded: false,
        failureClass: 'PUBLISH_CONFLICT',
        failedReleaseStage: 'PUBLISH',
      })
      return {
        commandType: envelope.commandType,
        outcome: 'success',
        message: result.reason,
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
