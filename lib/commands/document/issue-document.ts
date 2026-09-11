// ---------------------------------------------------------------------------
// DOC-06 — Canonical command wrapper: document.issue.
//
// The transaction belongs to the COMMAND: the issuance, its receipt and the
// command's own receipt must commit together, so this handler calls the vault
// repository's issuance function directly and passes `ctx.run`. Service/request
// surfaces go through VaultService instead (lib/vault-io.ts).
// ---------------------------------------------------------------------------

import { issueFormDocument } from '../../../db/issued-document'
import type {
  CommandEnvelope,
  CommandExecutionContext,
  CommandHandler,
  CommandResult,
} from '../contracts'
import { DOCUMENT_ISSUE } from '../command-types'

export { DOCUMENT_ISSUE }

export class IssueDocumentCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const { formInstanceId } = envelope.input as { formInstanceId?: string }
    if (!formInstanceId) {
      return {
        commandId: envelope.commandId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: envelope.aggregateId,
        message: 'document.issue requires formInstanceId.',
        replayed: false,
      }
    }
    return issueFormDocument(
      {
        commandId: envelope.commandId,
        formInstanceId,
        actorAppUserId: envelope.actorAppUserId ?? null,
        issuedAt: envelope.requestedAt,
      },
      ctx.run,
    )
  }
}
