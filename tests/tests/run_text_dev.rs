//! `run_text` renders what the database returns — against DEV, because only a
//! real driver hands back real column types.
//!
//! Run explicitly with:
//!   DATABASE_URL_DEV=... cargo test -p test-harness --test run_text_dev -- --ignored
//! Read-only: one SELECT of literals, no tables touched.
//!
//! WHY THIS EXISTS. `cell_as_text` (`db/src/pool.rs`) used to know four types
//! (text, bigint, int, bool) and rendered everything else as the empty string,
//! so a Forge `sql` answer silently dropped timestamps, uuids and json. The
//! arms below are the contract; the remaining gaps (NUMERIC, UUID) are
//! documented in `pool.rs` with the sqlx feature-gate reason and the `::text`
//! workaround, and the last two assertions pin that contract.

use db::{
    Database, DbTarget, LAUNCH_FLIGHT_FIRE_STORIES_SQL, LAUNCH_FLIGHT_QUEUE_ITEMS_SQL,
    LAUNCH_FLIGHT_ROUTE_ITEMS_SQL,
};

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn run_text_renders_every_common_type() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("the DEV pooler must accept this connection");
    let out = database
        .run_text(
            "select 'hi' as t, 42::bigint as b, 7::smallint as s, 1.5::float8 as f, \
             true as ok, null::text as n, '2026-01-02T03:04:05+00:00'::timestamptz as ts, \
             '2026-03-04'::date as d, '{\"a\":1}'::jsonb as j",
        )
        .await
        .expect("a SELECT of literals must render");
    assert_eq!(
        out,
        "hi|42|7|1.5|t||2026-01-02T03:04:05+00:00|2026-03-04|{\"a\":1}"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn run_text_uuid_and_numeric_need_a_text_cast() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("the DEV pooler must accept this connection");
    // The documented workaround renders.
    let out = database
        .run_text("select gen_random_uuid()::text as id, 19.99::numeric::text as price")
        .await
        .expect("casts must render");
    let mut parts = out.split('|');
    assert_eq!(parts.next().unwrap().len(), 36, "a uuid renders as text");
    assert_eq!(parts.next().unwrap(), "19.99");
    // ... while the bare types still vanish: sqlx needs its `uuid` /
    // `bigdecimal` cargo features to decode them (see `cell_as_text`).
    let out = database
        .run_text("select gen_random_uuid() as id, 19.99::numeric as price")
        .await
        .expect("bare types must not fail, only vanish");
    assert_eq!(out, "|", "bare uuid and numeric render empty");
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn launch_flight_statements_parse_against_the_live_schema() {
    // `EXPLAIN` plans without executing: this proves the set-based flight
    // statements name real tables and columns on the current schema, without
    // firing a batch or touching any story. `$1`/`$2` become literals because
    // `run_text` takes no binds.
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("the DEV pooler must accept this connection");
    let nil = "00000000-0000-0000-0000-000000000000";
    for (name, sql) in [
        ("fire", LAUNCH_FLIGHT_FIRE_STORIES_SQL),
        ("queue", LAUNCH_FLIGHT_QUEUE_ITEMS_SQL),
        ("route", LAUNCH_FLIGHT_ROUTE_ITEMS_SQL),
    ] {
        let planned = sql
            .replace("$1::uuid", &format!("'{nil}'::uuid"))
            .replace("$2", "'cheap'");
        database
            .run_text(&format!("explain {planned}"))
            .await
            .unwrap_or_else(|_| panic!("launch_flight {name} must plan on DEV"));
    }
}
