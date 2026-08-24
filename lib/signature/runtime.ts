import { commandDispatcher } from "@/lib/commands"
import { neonTx } from "@/db/tx"
import { SignatureApplication } from "@/lib/signature/application"
import { createBoldSignProvider } from "@/lib/signature/boldsign"
import { SignatureReconciliationHandler } from "@/lib/signature/reconciliation"
import { evaluateAgreementViaCommand } from "@/lib/agreements/re-drive"

let application: SignatureApplication | null = null

export function getSignatureApplication(): SignatureApplication {
  if (!application) {
    const provider = createBoldSignProvider()
    application = new SignatureApplication({
      dispatcher: commandDispatcher,
      provider,
      reconciler: new SignatureReconciliationHandler({
        provider,
        run: neonTx,
        // CRM-27 (F): after a completed event is reconciled, re-drive the
        // agreement-execution evaluation through the durable canonical command
        // (idempotent, exactly-once outbox emission) instead of a fragile
        // post-commit callback.
        evaluateAgreement: (documentId, eventId) =>
          evaluateAgreementViaCommand({ dispatcher: commandDispatcher }, documentId, eventId),
      }),
    })
  }
  return application
}
