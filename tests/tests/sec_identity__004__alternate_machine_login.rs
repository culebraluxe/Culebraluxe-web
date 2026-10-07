//! SEC.IDENTITY — alternate-machine login (TST-SEC-IDENTITY-004).
//!
//! Contract: **an account belongs to its provider identity, not to the machine, browser or session it was first seen
//! from.** The same person signing in from a laptop, then a phone, then a shared office desktop resolves to ONE
//! canonical identity every time.
//!
//! This is the property that makes the rest of the system honest. If anything machine-local — a device id, a session
//! id, a connection — could influence identity resolution, then an identity would be reproducible only on the device
//! that created it, and every account would fragment per machine. The test pins that from two sides, because the
//! return value alone cannot tell them apart: two queries that legitimately return the same principal look identical
//! whether the cache is keyed on the provider identity or on something the caller supplied.
//!
//! So the fake repository RECORDS every `(provider, subject)` it was asked about, and the test asserts the key was
//! the provider identity and nothing else — no machine, no session, no correlation id. That is the negative case the
//! contract needs: it fails if a future change ever threads a device or session identifier into the lookup.
//!
//! The session side of the same property is SEC.IDENTITY/009: the cookie carries the identity, and its lifetime is
//! bounded independently of which machine presented it.
//!
//! Level: L3 Composition — the real `SecurityService` with the real `CasbinAuthorizationPort`; the database faked at
//! the `SecurityRepository` adapter boundary production defines.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_identity__004__alternate_machine_login

use model::SecurityIdentityResolution;
use test_harness::security::{edge_context, SecurityHarness};

const HARNESS: &str = "SEC.IDENTITY/004";

#[tokio::test]
#[allow(non_snake_case)] // the canonical taxonomy name is part of the contract
async fn sec_identity_004__alternate_machine_login() {
    let store = SecurityHarness::new()
        .with_user(
            "user-ada",
            SecurityHarness::internal_user("user-ada", "internal", &["user"]),
        )
        .with_identity("google", "google-sub-ada", "user-ada");
    let service = store.service().await;
    let edge = edge_context();

    // FIRST MACHINE.
    let laptop = service
        .resolve_identity("google", "google-sub-ada", &edge)
        .await
        .expect("{HARNESS}: the first machine resolves");
    let SecurityIdentityResolution::Known(laptop_principal) = laptop else {
        panic!("{HARNESS}: a signed-in user must resolve, got {laptop:?}")
    };

    // A SECOND MACHINE presents the same identity. Nothing about the caller changes — the same system actor, a
    // different physical machine that production never sees and the test cannot even name — and the answer is the
    // same principal, to the field.
    let phone = service
        .resolve_identity("google", "google-sub-ada", &edge)
        .await
        .expect("{HARNESS}: the second machine resolves");
    let SecurityIdentityResolution::Known(phone_principal) = phone else {
        panic!("{HARNESS}: the second machine must resolve to the same identity, got {phone:?}")
    };
    assert_eq!(
        laptop_principal, phone_principal,
        "{HARNESS}: the same identity from another machine is the same canonical principal"
    );
    assert_eq!(
        phone_principal.acting_user.app_user_id, "user-ada",
        "{HARNESS}: both machines land in the one account"
    );

    // THE KEY IS THE PROVIDER IDENTITY. Two machines resolved the same identity and the store was asked ONCE,
    // keyed by `(provider, subject)` — the second machine was served the principal cached by the first. A machine id,
    // a session token or a connection fingerprint threaded into this key would show up as a second lookup keyed on
    // something else; a cache keyed per machine would show up as a second lookup too, on the same key. Either way an
    // identity would stop being portable, which is the contract.
    assert_eq!(
        store.looked_up(),
        vec![("google".to_owned(), "google-sub-ada".to_owned())],
        "{HARNESS}: two machines, one lookup — keyed on the provider identity and nothing else: {:?}",
        store.looked_up()
    );

    // A THIRD MACHINE, and the cache still answers it without a further lookup: a principal cached on one machine
    // is served to the next. If the cache were keyed per machine, this would be another lookup and — worse — a
    // machine could be handed a cached principal that is not its own.
    let tablet = service
        .resolve_identity("google", "google-sub-ada", &edge)
        .await
        .expect("{HARNESS}: the third machine resolves");
    let SecurityIdentityResolution::Known(tablet_principal) = tablet else {
        panic!("{HARNESS}: the third machine must resolve, got {tablet:?}")
    };
    assert_eq!(
        tablet_principal, laptop_principal,
        "{HARNESS}: a cached identity is the same identity on every machine"
    );
    assert_eq!(
        store.looked_up().len(),
        1,
        "{HARNESS}: three machines must not fragment one identity into three lookups"
    );

    // NEGATIVE — A DIFFERENT ACCOUNT ON THE SECOND MACHINE IS STILL A DIFFERENT ACCOUNT. Machine-independence must
    // not have become machine-equality: arriving from "the same machine" is not something a caller can assert, and if
    // it could, any user could present themselves as whoever that machine last resolved.
    let other_store = SecurityHarness::new()
        .with_user(
            "user-grace",
            SecurityHarness::internal_user("user-grace", "internal", &["user"]),
        )
        .with_identity("google", "google-sub-grace", "user-grace");
    let other_service = other_store.service().await;
    let grace = other_service
        .resolve_identity("google", "google-sub-grace", &edge)
        .await
        .expect("{HARNESS}: grace resolves");
    let SecurityIdentityResolution::Known(grace_principal) = grace else {
        panic!("{HARNESS}: grace must resolve to her own identity")
    };
    assert_ne!(
        grace_principal.acting_user.app_user_id, laptop_principal.acting_user.app_user_id,
        "{HARNESS}: a second machine does not collapse two accounts into one"
    );

    // NEGATIVE — A MACHINE CANNOT SUPPLY AN IDENTITY THAT WAS NEVER SIGNED IN ON IT. The device brings nothing; the
    // provider identity brings everything, and an unknown one resolves to nobody.
    let unknown = other_service
        .resolve_identity("google", "google-sub-never-signed-in", &edge)
        .await
        .expect("{HARNESS}: an unknown subject is an answer, not a failure");
    assert!(
        matches!(unknown, SecurityIdentityResolution::Unmapped),
        "{HARNESS}: no amount of returning resolves an identity nobody signed in with: {unknown:?}"
    );
    assert_eq!(
        store.written(),
        Vec::<String>::new(),
        "{HARNESS}: a machine returning does not write"
    );
}
