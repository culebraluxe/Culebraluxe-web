import type { QueryExecutor } from '@/legacy/db/query-executor'
import { FORGE_FAILURE_CLASSES, isForgeFailureClass } from '@/legacy/workflow_app/forge/failure-classifier'
import { analyzeFailure, requestTypeSafe, triageRequest, triageText, TriageInputError } from '@/legacy/workflow_app/forge/typesafe-failure-triage'
import { listTriageSources, readTriageSource, saveTriageObservation, reviewTriage, triageReport } from '@/legacy/db/forge-typesafe-triage'
import { recordError } from '@/legacy/db/app-error'

const HELP = `Forge TypeSafe triage — advisory only; no engine routing or retries.
  pnpm forge:triage list [STORY-ID]
  pnpm forge:triage preview ARTIFACT-UUID
  pnpm forge:triage analyze ARTIFACT-UUID
  pnpm forge:triage review TRIAGE-UUID CLASS "confirmed evidence"
  pnpm forge:triage report [STORY-ID]
Classes: ${FORGE_FAILURE_CLASSES.join(', ')}
analyze sends the previewed, bounded evidence to TypeSafe and records an observation.
review records your confirmed class; report compares reviewed cases.
Set TYPESAFE_API_KEY in .env.local for analyze. No key needed for other commands.`

export function validateTriageArgs(args: string[]) {
  const [command] = args
  if (!command || command === '--help') return 'help'
  const valid = (command === 'list' || command === 'report') ? args.length <= 2
    : (command === 'preview' || command === 'analyze') ? args.length === 2
      : command === 'review' ? args.length === 4 && isForgeFailureClass(args[2]) && Boolean(args[3].trim()) : false
  if (!valid) throw new TriageInputError(HELP)
  return command
}

export async function runTriageCommand(args: string[], q: QueryExecutor, apiKey = process.env.TYPESAFE_API_KEY ?? '') {
  const command = validateTriageArgs(args)
  if (command === 'help') return HELP
  if (command === 'list') return listTriageSources(q, args[1] ?? null)
  if (command === 'report') return triageReport(q, args[1] ?? null)
  if (command === 'review') {
    if (!isForgeFailureClass(args[2])) throw new TriageInputError('Unknown class.')
    return reviewTriage(q, args[1], args[2], args[3])
  }
  if (command === 'analyze' && !apiKey.trim()) throw new TriageInputError('Set TYPESAFE_API_KEY in .env.local.')
  const source = await readTriageSource(q, args[1])
  if (command === 'preview') return triageRequest(source)
  return analyzeFailure(source, {
    evaluate: request => requestTypeSafe(request, apiKey),
    save: (s, o) => saveTriageObservation(q, s, o),
  })
}

async function main() {
  let q: QueryExecutor | undefined
  try {
    const args = process.argv.slice(2)
    if (validateTriageArgs(args) === 'help') {
      console.log(HELP)
      return
    }
    if (process.env.APP_ENV !== 'production' || process.env.EXECUTION_ENV !== 'PROD') {
      throw new TriageInputError('Use pnpm forge:triage: the Forge control plane must be PROD.')
    }
    const { sql } = await import('@/legacy/db/client')
    q = sql
    console.log(JSON.stringify(await runTriageCommand(args, q), null, 2))
  } catch (err) {
    const message = triageText(err instanceof Error ? err.message : 'Triage failed', 1600)
    if (!(err instanceof TriageInputError)) {
      try {
        await recordError({ kind: 'TypeSafeTriageError', operation: 'forge:triage', message, level: 'error' }, q)
      } catch {
        console.error('Durable error capture also failed; the operation was not completed.')
      }
    }
    console.error(message)
    process.exitCode = 1
  }
}

if (process.argv[1] && /(^|\/)forge-triage\.ts$/.test(process.argv[1])) {
  void main().then(() => process.exit(process.exitCode ?? 0))
}
