//! The applied-signature resolver, moved out of `mod.rs` (2026-10-14).
//!
//! `mod.rs` had grown past the boundedness ceiling and this method is a self-contained query: it resolves the
//! brokerage's standing pre-signature exactly as issuance resolves it, writing nothing. The move is text-exact
//! and the `impl` block repeats the parent's generics, so `VaultService::resolve_applied_signatures` is still
//! the same path for its caller (`api/portal_bridge/forms_values.rs`).

use super::*;

impl<R: VaultRepository> VaultService<R> {
    /// The brokerage's standing pre-signature, resolved exactly as issuance resolves it, for a caller that only wants
    /// to SHOW it.
    ///
    /// The preview pane and the issued document must draw the same signature, and that is why both ask this one
    /// resolver: the pane that resolves it for itself drifts from the document the moment either side changes
    /// (`docs/agent/HANDOFF-luxesign-native-2026-10-07.md`).
    ///
    /// It is a query: nothing is applied, nothing is written and no version moves. It resolves under
    /// `OwnerByConstruction`, so the person looking at the draft is not required to be the person who will sign it —
    /// while every other rule (the template's policy, the declared-signer check, the slot mapping, the protected
    /// asset) is the rule issuance obeys, so the pane cannot show what issuance would not apply.
    ///
    /// A REFUSAL is passed on rather than flattened to an empty list: the resolver only refuses what makes an issuance
    /// impossible, and a preview that silently drew nothing there would hide the very defect that makes the document
    /// unissuable.
    pub async fn resolve_applied_signatures(
        &self,
        request: &VaultRenderRequest,
        context: &ServiceContext,
    ) -> Result<Vec<FormAppliedSignature>, CoreServiceError> {
        const OP: &str = "vault.resolveAppliedSignatures";
        let decision = authorize(
            &self.runtime,
            "vault",
            "vault.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;

        let result = self
            .repository
            .resolve_applied_signatures(request)
            .await
            .map_err(|failure| match failure.outcome {
                // The rows were unreachable or the connection could not be acquired: infrastructure, and captured.
                VaultCommandOutcome::PreconditionFailure => CoreServiceError::infrastructure(
                    "VAULT_SIGNATURE_LOOKUP_FAILED",
                    failure.message,
                ),
                // Policy refused it. The document cannot carry her signature as configured — a business refusal the
                // caller must see, not error noise.
                _ => CoreServiceError::business("VAULT_SIGNATURE_REFUSED", failure.message),
            });

        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }
}
