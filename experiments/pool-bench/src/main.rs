//! Apples-to-apples: the same statements through sqlx::PgPool and through
//! tokio-postgres + deadpool, on the same machine, against the same database, in the same minute.
//!
//! Run with DATABASE_URL_DEV in the environment, e.g.:
//!   DATABASE_URL_DEV="$(grep -m1 '^DATABASE_URL_DEV=' .env.local | cut -d= -f2-)" cargo run --release

use std::time::{Duration, Instant};

fn median(mut xs: Vec<Duration>) -> Duration {
    xs.sort();
    xs[xs.len() / 2]
}

/// The same, without consuming the samples, so a set can be reported twice.
fn median_of(xs: &[Duration]) -> Duration {
    let mut sorted = xs.to_vec();
    sorted.sort();
    sorted[sorted.len() / 2]
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL_DEV").expect("DATABASE_URL_DEV must be set");
    let rounds = 12usize;

    // ---- sqlx::PgPool -------------------------------------------------------------------------------
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .min_connections(1)
        .test_before_acquire(false) // our production setting: no round trip per checkout
        .connect(&url)
        .await?;

    let mut sqlx_select = Vec::new();
    for _ in 0..rounds {
        let started = Instant::now();
        let _: i32 = sqlx::query_scalar("select 1::int").fetch_one(&pool).await?;
        sqlx_select.push(started.elapsed());
    }

    let mut sqlx_tx = Vec::new();
    for _ in 0..rounds {
        let started = Instant::now();
        let mut tx = pool.begin().await?;
        let _: i32 = sqlx::query_scalar("select 1::int").fetch_one(&mut *tx).await?;
        tx.commit().await?;
        sqlx_tx.push(started.elapsed());
    }

    // ---- tokio-postgres + deadpool ------------------------------------------------------------------
    let pg_config: tokio_postgres::Config = url.parse()?;
    let tls = postgres_native_tls::MakeTlsConnector::new(native_tls::TlsConnector::builder().build()?);
    // ManagerConfig::default() is RecyclingMethod::Fast - the deadpool default, no round trip on recycle.
    let mgr = deadpool_postgres::Manager::new(pg_config, tls);
    let dead_pool = deadpool_postgres::Pool::builder(mgr).max_size(5).build()?;

    // One checkout first, so the timings below are about reuse rather than the first connect.
    let _warm = dead_pool.get().await?;

    let mut dp_select = Vec::new();
    for _ in 0..rounds {
        let started = Instant::now();
        let client = dead_pool.get().await?;
        let _row = client.query_one("select 1::int", &[]).await?;
        dp_select.push(started.elapsed());
    }

    let mut dp_tx = Vec::new();
    for _ in 0..rounds {
        let started = Instant::now();
        let mut client = dead_pool.get().await?;
        let tx = client.transaction().await?;
        let _row = tx.query_one("select 1::int", &[]).await?;
        tx.commit().await?;
        dp_tx.push(started.elapsed());
    }

    // Is the 2x the checkout, or the query? Time them separately.
    let mut dp_get = Vec::new();
    for _ in 0..rounds {
        let started = Instant::now();
        let _client = dead_pool.get().await?;
        dp_get.push(started.elapsed());
    }

    let mut dp_held = Vec::new();
    {
        let client = dead_pool.get().await?;
        for _ in 0..rounds {
            let started = Instant::now();
            let _row = client.query_one("select 1::int", &[]).await?;
            dp_held.push(started.elapsed());
        }
    }

    let mut sqlx_held = Vec::new();
    {
        let mut conn = pool.acquire().await?;
        for _ in 0..rounds {
            let started = Instant::now();
            let _: i32 = sqlx::query_scalar("select 1::int").fetch_one(&mut *conn).await?;
            sqlx_held.push(started.elapsed());
        }
    }

    // What we actually run in production: no statement cache, because the endpoint is a transaction-mode pooler.
    // If this is twice the cached number, every statement in production is paying two round trips.
    let uncached = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .min_connections(1)
        .test_before_acquire(false)
        .connect_with(
            url.parse::<sqlx::postgres::PgConnectOptions>()?
                .statement_cache_capacity(0),
        )
        .await?;
    let mut sqlx_uncached = Vec::new();
    {
        let mut conn = uncached.acquire().await?;
        for _ in 0..rounds {
            let started = Instant::now();
            let _: i32 = sqlx::query_scalar("select 1::int").fetch_one(&mut *conn).await?;
            sqlx_uncached.push(started.elapsed());
        }
    }

    println!(
        "sqlx cache on/off  held conn with cache={:?}  no statement cache (production setting)={:?}",
        median_of(&sqlx_held),
        median(sqlx_uncached)
    );

    println!(
        "decomposed        deadpool get() alone={:?}  deadpool query on held conn={:?}  sqlx query on held conn={:?}",
        median(dp_get),
        median(dp_held),
        median(sqlx_held)
    );
    println!(
        "select 1          sqlx(PgPool)={:?}  deadpool+tokio-postgres={:?}",
        median(sqlx_select),
        median(dp_select)
    );
    println!(
        "begin/select/commit sqlx(PgPool)={:?}  deadpool+tokio-postgres={:?}",
        median(sqlx_tx),
        median(dp_tx)
    );
    let (sqlx_pool, dp_pool) = (pool.size(), dead_pool.status().size);
    println!("pool size         sqlx={sqlx_pool}  deadpool={dp_pool}");
    Ok(())
}
