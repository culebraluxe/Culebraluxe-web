import { commandDispatcher } from "@/lib/commands"
import { neonTx } from "@/db/tx"
import { SignatureApplication } from "@/lib/signature/application"
import { createBoldSignProvider } from "@/lib/signature/boldsign"
import { SignatureReconciliationHandler } from "@/lib/signature/reconciliation"

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
      }),
    })
  }
  return application
}
