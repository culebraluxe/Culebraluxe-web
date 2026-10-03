//! The pool's connection contract against the DEV database — the endpoint is a Neon `-pooler` (PgBouncer), and that
//! is the whole point: this test opens a real socket to it. Ignored by default; run with
//!   DATABASE_URL_DEV=... cargo test -p db --test pool_connect_dev -- --ignored
//! Read-only: it opens connections and reads settings, and changes no data.
//!
//! WHY THIS EXISTS. The per-statement ceiling was applied as a libpq startup option
//! (`PgConnectOptions::options([("statement_timeout", …)])`). PgBouncer refuses a startup option it does not track,
//! so EVERY connection through the pooled endpoint died before a single query ran:
//!
//!   unsupported startup parameter in options: statement_timeout (SQLSTATE 08P01)
//!
//! Nothing in the unit suite opens a socket, so a green `cargo test` said nothing about it; it was found by starting
//! the server against DEV. The first assertion below fails at CONNECT for that mistake, and the second fails if the
//! ceiling only exists at handshake and is gone by the next transaction — the two ways this can silently not work on
//! a pooled endpoint.

use db::{Database, DbTarget};

async fn setting(database: &Database) -> String {
    sqlx::query_scalar::<_, String>("select current_setting('statement_timeout')")
        .fetch_one(database.pool())
        .await
        .expect("current_setting must be readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn the_pool_connects_to_a_pooled_endpoint_and_the_ceiling_survives() {
    // 1. THE CONNECT ITSELF. A startup parameter the pooler refuses fails here, with the driver's message.
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("the DEV pooler must accept this connection");
    assert_eq!(database.target().as_str(), "dev");

    // 2. THE CEILING IS IN EFFECT, READ FROM THE SERVER rather than from the code that set it.
    let first = setting(&database).await;
    assert_ne!(
        first, "0",
        "the statement ceiling is not applied on connect: a runaway statement would hold its connection"
    );

    // 3. AND IT IS STILL THERE ON A LATER CHECKOUT. Transaction pooling hands out server connections per
    // transaction, so a setting that evaporates loses the protection exactly where it is wanted.
    setting(&database).await;
    let later = setting(&database).await;
    assert_eq!(
        first, later,
        "the ceiling changed between checkouts on a pooled endpoint"
    );

    // 4. A LIVE STATEMENT IS ACTUALLY CUT OFF, which is the behaviour all of the above is for. An operator's own
    // long work turns it off (`db::disable_statement_timeout`), and this asserts the other side of that switch: with
    // the ceiling on, `pg_sleep` past it is cancelled.
    sqlx::query("select pg_sleep(40)")
        .execute(database.pool())
        .await
        .expect_err("a 40s sleep must not outlive the 30s ceiling");
}
