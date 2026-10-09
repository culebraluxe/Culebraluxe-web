//! DB.CONCURRENCY — two entitlement updates (TST-DB-CONCURRENCY-014).
//!
//! Contract: two concurrent entitlement updates on the same role converge to one
//! legal durable state. `SecurityDao::set_role_entitlement` performs the grant
//! change in one statement (`insert ... on conflict do nothing` / `delete using
//! target`), so a grant racing a revoke must never yield a duplicate row, a lost
//! half-update, or a refused-yet-applied change: the role ends with the
//! entitlement present or absent, exactly.
//!
//! Level: L4 Adversarial — barrier-rendezvous concurrency plus injected faults.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_concurrency__014__two_entitlement_updates -- --ignored

use db::{Database, DbTarget, SecurityDao};
use std::sync::Arc;
use test_harness::barrier::ConcurrencyBarrier;
use test_harness::fault::{Fault, FaultInjector};
use uuid::Uuid;

async fn role_code() -> String {
    format!("tst-014-role-{}", Uuid::new_v4().simple())
}

async fn create_role(db: &Database, code: &str) {
    sqlx::query(
        "insert into security_role (code, name, account_type, description) values ($1, 'TST-014', 'internal', 'scratch role')",
    )
    .bind(code)
    .execute(db.pool())
    .await
    .expect("role fixture");
}

async fn drop_role(db: &Database, code: &str) {
    sqlx::query("delete from security_role where code = $1")
        .bind(code)
        .execute(db.pool())
        .await
        .ok();
}

async fn grant_count(db: &Database, code: &str, action: &str) -> i64 {
    sqlx::query_scalar(
        "select count(*) from role_entitlement re \
         join security_role r on r.id = re.role_id \
         join entitlement e on e.id = re.entitlement_id \
         where r.code = $1 and e.code = $2",
    )
    .bind(code)
    .bind(action)
    .fetch_one(db.pool())
    .await
    .expect("grant count")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn db_concurrency_014__two_entitlement_updates() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let dao = Arc::new(SecurityDao::new(db.clone()));
    let action = "deal.read";

    // Negative case: an unknown role refuses the change, it does not silently apply.
    let missing = dao
        .set_role_entitlement(&role_code().await, action, true)
        .await
        .expect("set_role_entitlement");
    assert!(
        !missing,
        "an unknown role must report the change did not apply"
    );

    // Test 1: Both workers grant the same entitlement.
    let code_1 = role_code().await;
    create_role(&db, &code_1).await;
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (dao, barrier, code_1) = (dao.clone(), barrier.clone(), code_1.clone());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            dao.set_role_entitlement(&code_1, action, true).await
        }));
    }
    for handle in handles {
        assert!(
            handle.await.expect("racer panicked").expect("grant"),
            "both workers resolve the same real role and entitlement"
        );
    }
    assert_eq!(
        grant_count(&db, &code_1, action).await,
        1,
        "one grant row, never two"
    );

    // Test 2: A grant racing a revoke on the same role converges to exactly one
    // legal state: the entitlement row present once, or absent — no duplicates,
    // no half-applied change.
    let code_2 = role_code().await;
    create_role(&db, &code_2).await;
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for granted in [true, false] {
        let (dao, barrier, code_2) = (dao.clone(), barrier.clone(), code_2.clone());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            dao.set_role_entitlement(&code_2, action, granted).await
        }));
    }
    for handle in handles {
        handle.await.expect("racer panicked").expect("update");
    }
    let count = grant_count(&db, &code_2, action).await;
    assert!(
        count == 0 || count == 1,
        "the grant must be fully present or fully absent, got {count}"
    );

    // Test 3: Fault injection — one worker dies mid-update; the survivor's
    // change still lands, and the row count stays legal.
    let code_3 = role_code().await;
    create_role(&db, &code_3).await;
    let injector = Arc::new(FaultInjector::scripted(vec![
        Fault::None,
        Fault::error("CHAOS_CRASH", "worker died during entitlement update"),
    ]));
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (dao, barrier, injector, code_3) = (
            dao.clone(),
            barrier.clone(),
            injector.clone(),
            code_3.clone(),
        );
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            if injector.next_fault().is_failure() {
                return Err("crashed".to_string());
            }
            dao.set_role_entitlement(&code_3, action, true)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        }));
    }
    let mut success = 0;
    for handle in handles {
        if handle.await.expect("racer panicked").is_ok() {
            success += 1;
        }
    }
    assert_eq!(success, 1, "the survivor still applies its update");
    assert_eq!(
        grant_count(&db, &code_3, action).await,
        1,
        "the survivor's grant must be durable"
    );

    drop_role(&db, &code_1).await;
    drop_role(&db, &code_2).await;
    drop_role(&db, &code_3).await;
}
