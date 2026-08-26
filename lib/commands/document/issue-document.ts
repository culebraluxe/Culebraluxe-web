// ---------------------------------------------------------------------------
// DOC-06 — Canonical command wrapper: document.issue.
//
// Thin adapter over the canonical issuance service db/issued-document.ts
// (issueFormDocument). The service owns legality, the claim-first receipt and
// the ONE-transaction mutation (render PDF → append media → insert the
// immutable transaction_document → mark the form issued); this handler only
// translates the envelope into the service call. No business rules live here.
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
