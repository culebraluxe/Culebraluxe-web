//! The service mailbox's behaviour: bounded queue, concurrency, timeouts, drain and shutdown, panics, cancellation.
use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn typed_tasks_use_the_same_mailbox_without_json_coercion() {
    let root = CancellationToken::new();
    let mailbox =
        ServiceMailbox::spawn("contract", ServiceMailboxConfig::default(), &root).unwrap();

    let result: usize = mailbox
        .submit_task(
            "contract.count",
            ServiceExecutionPolicy::inline(),
            &json!({}),
            async { 42usize },
        )
        .await
        .unwrap();

    assert_eq!(result, 42);
    mailbox.stop().await.unwrap();
}

#[tokio::test]
async fn ordered_work_serializes_one_partition_but_allows_other_partitions() {
    let root = CancellationToken::new();
    let mailbox = ServiceMailbox::spawn(
        "property",
        ServiceMailboxConfig {
            capacity: 8,
            max_concurrency: 2,
        },
        &root,
    )
    .unwrap();

    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let mut jobs = Vec::new();

    for property_id in ["p1", "p1", "p2"] {
        let mailbox = mailbox.clone();
        let active = active.clone();
        let peak = peak.clone();
        let payload = json!({ "propertyId": property_id });
        jobs.push(tokio::spawn(async move {
            mailbox
                .submit(
                    "property.save",
                    ServiceExecutionPolicy::ordered("propertyId"),
                    &payload,
                    async move {
                        let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(now, Ordering::SeqCst);
                        sleep(Duration::from_millis(25)).await;
                        active.fetch_sub(1, Ordering::SeqCst);
                        Ok(json!({ "ok": true }))
                    },
                )
                .await
                .unwrap();
        }));
    }

    for job in jobs {
        job.await.unwrap();
    }

    assert_eq!(peak.load(Ordering::SeqCst), 2);
    mailbox.stop().await.unwrap();
}

#[tokio::test]
async fn bounded_mailbox_applies_backpressure_to_waiting_work() {
    let root = CancellationToken::new();
    let mailbox = ServiceMailbox::spawn(
        "bounded",
        ServiceMailboxConfig {
            capacity: 1,
            max_concurrency: 1,
        },
        &root,
    )
    .unwrap();

    let first_release = Arc::new(Notify::new());
    let second_release = Arc::new(Notify::new());
    let first_started = Arc::new(Notify::new());
    let second_started = Arc::new(Notify::new());
    let third_started = Arc::new(Notify::new());

    let first = {
        let mailbox = mailbox.clone();
        let release = first_release.clone();
        let started = first_started.clone();
        tokio::spawn(async move {
            mailbox
                .submit(
                    "bounded.work",
                    ServiceExecutionPolicy::queued(),
                    &json!({}),
                    async move {
                        started.notify_one();
                        release.notified().await;
                        Ok(json!({ "n": 1 }))
                    },
                )
                .await
        })
    };
    first_started.notified().await;

    let second = {
        let mailbox = mailbox.clone();
        let release = second_release.clone();
        let started = second_started.clone();
        tokio::spawn(async move {
            mailbox
                .submit(
                    "bounded.work",
                    ServiceExecutionPolicy::queued(),
                    &json!({}),
                    async move {
                        started.notify_one();
                        release.notified().await;
                        Ok(json!({ "n": 2 }))
                    },
                )
                .await
        })
    };

    // Let the actor enqueue the second task; its queue permit remains held.
    sleep(Duration::from_millis(10)).await;

    let third = {
        let mailbox = mailbox.clone();
        let started = third_started.clone();
        tokio::spawn(async move {
            mailbox
                .submit(
                    "bounded.work",
                    ServiceExecutionPolicy::queued(),
                    &json!({}),
                    async move {
                        started.notify_one();
                        Ok(json!({ "n": 3 }))
                    },
                )
                .await
        })
    };

    assert!(
        tokio::time::timeout(Duration::from_millis(20), third_started.notified())
            .await
            .is_err(),
        "third task crossed the bounded mailbox while one task was already queued"
    );

    first_release.notify_one();
    second_started.notified().await;
    assert!(
        tokio::time::timeout(Duration::from_millis(20), third_started.notified())
            .await
            .is_err(),
        "third task started while the second task still held the only execution slot"
    );

    second_release.notify_one();
    third_started.notified().await;

    assert!(first.await.unwrap().is_ok());
    assert!(second.await.unwrap().is_ok());
    assert!(third.await.unwrap().is_ok());
    mailbox.stop().await.unwrap();
}

#[tokio::test]
async fn drain_refuses_new_work_and_waits_for_accepted_work() {
    let root = CancellationToken::new();
    let mailbox =
        ServiceMailbox::spawn("contract", ServiceMailboxConfig::default(), &root).unwrap();
    let (started_tx, started_rx) = oneshot::channel();

    let running = mailbox.clone();
    let accepted = tokio::spawn(async move {
        running
            .submit(
                "contract.execute",
                ServiceExecutionPolicy::queued(),
                &json!({}),
                async move {
                    let _ = started_tx.send(());
                    sleep(Duration::from_millis(20)).await;
                    Ok(json!({ "executed": true }))
                },
            )
            .await
    });

    started_rx.await.unwrap();
    mailbox.drain().await.unwrap();
    assert!(accepted.await.unwrap().is_ok());
    assert_eq!(mailbox.health().in_flight, 0);
    assert!(matches!(
        mailbox
            .submit(
                "contract.execute",
                ServiceExecutionPolicy::queued(),
                &json!({}),
                async { Ok(json!({})) }
            )
            .await,
        Err(ServiceDispatchError::ServiceDraining(_))
    ));

    mailbox.cancel();
    mailbox.wait_stopped().await.unwrap();
}

#[tokio::test]
async fn actor_panic_enters_failed_and_releases_waiting_caller() {
    let root = CancellationToken::new();
    let mailbox = ServiceMailbox::spawn(
        "panic-proof",
        ServiceMailboxConfig {
            capacity: 4,
            max_concurrency: 1,
        },
        &root,
    )
    .unwrap();
    mailbox.wait_running().await.unwrap();

    let release = Arc::new(Notify::new());
    let started = Arc::new(Notify::new());
    let first = {
        let mailbox = mailbox.clone();
        let release = release.clone();
        let started = started.clone();
        tokio::spawn(async move {
            mailbox
                .submit(
                    "panic-proof.first",
                    ServiceExecutionPolicy::queued(),
                    &json!({}),
                    async move {
                        started.notify_one();
                        release.notified().await;
                        Ok(json!({ "ok": true }))
                    },
                )
                .await
        })
    };
    started.notified().await;

    let waiting = {
        let mailbox = mailbox.clone();
        tokio::spawn(async move {
            mailbox
                .submit(
                    "panic-proof.waiting",
                    ServiceExecutionPolicy::queued(),
                    &json!({}),
                    async { Ok(json!({ "never": "runs" })) },
                )
                .await
        })
    };
    sleep(Duration::from_millis(10)).await;

    mailbox.panic_actor_for_test().await;

    let failed = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if mailbox.status() == ServiceStatus::Failed {
                break;
            }
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(failed.is_ok(), "actor panic never reached Failed state");

    let waiting_result = tokio::time::timeout(Duration::from_secs(1), waiting)
        .await
        .expect("waiting caller deadlocked")
        .unwrap();
    assert!(matches!(
        waiting_result,
        Err(ServiceDispatchError::ServiceStopped(_))
    ));

    let first_result = tokio::time::timeout(Duration::from_secs(1), first)
        .await
        .expect("active caller was not released after actor failure")
        .unwrap();
    assert!(matches!(
        first_result,
        Err(ServiceDispatchError::OperationPanicked { .. })
    ));
    assert_eq!(mailbox.health().in_flight, 0);
}

#[tokio::test]
async fn parent_cancellation_drains_accepted_work_before_stop() {
    let root = CancellationToken::new();
    let mailbox =
        ServiceMailbox::spawn("cancel-proof", ServiceMailboxConfig::default(), &root).unwrap();
    mailbox.wait_running().await.unwrap();

    let started = Arc::new(Notify::new());
    let accepted = {
        let mailbox = mailbox.clone();
        let started = started.clone();
        tokio::spawn(async move {
            mailbox
                .submit(
                    "cancel-proof.work",
                    ServiceExecutionPolicy::queued(),
                    &json!({}),
                    async move {
                        started.notify_one();
                        sleep(Duration::from_millis(40)).await;
                        Ok(json!({ "committed": true }))
                    },
                )
                .await
        })
    };
    started.notified().await;

    root.cancel();

    assert!(
        accepted.await.unwrap().is_ok(),
        "accepted work was dropped by parent cancellation"
    );
    mailbox.wait_stopped().await.unwrap();
    assert_eq!(mailbox.status(), ServiceStatus::Stopped);
}

#[tokio::test]
async fn child_cancellation_does_not_cancel_parent() {
    let root = CancellationToken::new();
    let mailbox =
        ServiceMailbox::spawn("child-proof", ServiceMailboxConfig::default(), &root).unwrap();
    mailbox.wait_running().await.unwrap();

    mailbox.cancel();
    mailbox.wait_stopped().await.unwrap();

    assert!(
        !root.is_cancelled(),
        "child service cancellation propagated upward into the root token"
    );
}

#[tokio::test]
async fn repeated_drain_and_stop_are_deterministic() {
    let root = CancellationToken::new();
    let mailbox =
        ServiceMailbox::spawn("repeat-proof", ServiceMailboxConfig::default(), &root).unwrap();
    mailbox.wait_running().await.unwrap();

    mailbox.drain().await.unwrap();
    mailbox.drain().await.unwrap();
    mailbox.stop().await.unwrap();
    mailbox.stop().await.unwrap();

    assert_eq!(mailbox.status(), ServiceStatus::Stopped);
    assert!(!mailbox.health().accepting);
}

#[tokio::test]
async fn drain_tracks_inline_work_too() {
    let root = CancellationToken::new();
    let mailbox =
        ServiceMailbox::spawn("property", ServiceMailboxConfig::default(), &root).unwrap();
    let (started_tx, started_rx) = oneshot::channel();

    let running = mailbox.clone();
    let work = tokio::spawn(async move {
        running
            .submit(
                "property.get",
                ServiceExecutionPolicy::inline(),
                &json!({}),
                async move {
                    let _ = started_tx.send(());
                    sleep(Duration::from_millis(20)).await;
                    Ok(json!({ "id": "p1" }))
                },
            )
            .await
    });

    started_rx.await.unwrap();
    mailbox.drain().await.unwrap();
    assert!(work.await.unwrap().is_ok());
    assert_eq!(mailbox.health().in_flight, 0);

    mailbox.cancel();
    mailbox.wait_stopped().await.unwrap();
}

#[tokio::test]
async fn ordered_partition_preserves_fifo_when_other_partition_finishes_first() {
    let root = CancellationToken::new();
    let mailbox = ServiceMailbox::spawn(
        "fifo-proof",
        ServiceMailboxConfig {
            capacity: 8,
            max_concurrency: 2,
        },
        &root,
    )
    .unwrap();
    let release_first = Arc::new(Notify::new());
    let first_started = Arc::new(Notify::new());
    let order = Arc::new(tokio::sync::Mutex::new(Vec::new()));

    let spawn = |id: usize, partition: &'static str, wait: bool| {
        let mailbox = mailbox.clone();
        let release = release_first.clone();
        let started = first_started.clone();
        let order = order.clone();
        tokio::spawn(async move {
            mailbox
                .submit(
                    "fifo-proof.work",
                    ServiceExecutionPolicy::ordered("key"),
                    &json!({"key": partition}),
                    async move {
                        order.lock().await.push(id);
                        if id == 1 {
                            started.notify_one();
                        }
                        if wait {
                            release.notified().await;
                        }
                        Ok(json!({}))
                    },
                )
                .await
        })
    };

    let first = spawn(1, "a", true);
    first_started.notified().await;
    let second = spawn(2, "a", false);
    tokio::time::timeout(Duration::from_secs(1), async {
        while mailbox.health().queued < 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("second partition-a item was not admitted");
    let other = spawn(9, "b", false);
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if order.lock().await.contains(&9) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("independent partition did not start");
    let third = spawn(3, "a", false);
    sleep(Duration::from_millis(20)).await;
    release_first.notify_one();
    for job in [first, second, other, third] {
        assert!(job.await.unwrap().is_ok());
    }
    let order = order.lock().await.clone();
    let partition_a = order.into_iter().filter(|id| *id != 9).collect::<Vec<_>>();
    assert_eq!(partition_a, vec![1, 2, 3]);
    mailbox.stop().await.unwrap();
}

#[tokio::test]
async fn parent_cancellation_drains_work_buffered_before_the_fence() {
    let root = CancellationToken::new();
    let mailbox = ServiceMailbox::spawn(
        "buffered-cancel-proof",
        ServiceMailboxConfig {
            capacity: 4,
            max_concurrency: 1,
        },
        &root,
    )
    .unwrap();
    let release = Arc::new(Notify::new());
    let started = Arc::new(Notify::new());
    let first = {
        let mailbox = mailbox.clone();
        let release = release.clone();
        let started = started.clone();
        tokio::spawn(async move {
            mailbox
                .submit(
                    "buffered.first",
                    ServiceExecutionPolicy::queued(),
                    &json!({}),
                    async move {
                        started.notify_one();
                        release.notified().await;
                        Ok(json!({"first": true}))
                    },
                )
                .await
        })
    };
    started.notified().await;
    let second = {
        let mailbox = mailbox.clone();
        tokio::spawn(async move {
            mailbox
                .submit(
                    "buffered.second",
                    ServiceExecutionPolicy::queued(),
                    &json!({}),
                    async { Ok(json!({"second": true})) },
                )
                .await
        })
    };
    sleep(Duration::from_millis(10)).await;
    root.cancel();
    release.notify_one();
    assert!(first.await.unwrap().is_ok());
    assert!(second.await.unwrap().is_ok());
    mailbox.wait_stopped().await.unwrap();
}

#[tokio::test]
async fn inline_work_obeys_the_same_concurrency_limit() {
    let root = CancellationToken::new();
    let mailbox = ServiceMailbox::spawn(
        "inline-bound-proof",
        ServiceMailboxConfig {
            capacity: 8,
            max_concurrency: 1,
        },
        &root,
    )
    .unwrap();
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let mut jobs = Vec::new();
    for _ in 0..4 {
        let mailbox = mailbox.clone();
        let active = active.clone();
        let peak = peak.clone();
        jobs.push(tokio::spawn(async move {
            mailbox
                .submit(
                    "inline-bound.work",
                    ServiceExecutionPolicy::inline(),
                    &json!({}),
                    async move {
                        let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(now, Ordering::SeqCst);
                        sleep(Duration::from_millis(10)).await;
                        active.fetch_sub(1, Ordering::SeqCst);
                        Ok(json!({}))
                    },
                )
                .await
        }));
    }
    for job in jobs {
        assert!(job.await.unwrap().is_ok());
    }
    assert_eq!(peak.load(Ordering::SeqCst), 1);
    mailbox.stop().await.unwrap();
}

#[tokio::test]
async fn force_stop_aborts_hung_work_and_releases_its_resources() {
    struct DropProof(Arc<AtomicBool>);
    impl Drop for DropProof {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    let root = CancellationToken::new();
    let mailbox =
        ServiceMailbox::spawn("force-proof", ServiceMailboxConfig::default(), &root).unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let started = Arc::new(Notify::new());
    let work = {
        let mailbox = mailbox.clone();
        let dropped = dropped.clone();
        let started = started.clone();
        tokio::spawn(async move {
            mailbox
                .submit(
                    "force-proof.hung",
                    ServiceExecutionPolicy::queued(),
                    &json!({}),
                    async move {
                        let _proof = DropProof(dropped);
                        started.notify_one();
                        std::future::pending::<()>().await;
                        Ok(json!({}))
                    },
                )
                .await
        })
    };
    started.notified().await;
    mailbox.force_stop();
    assert!(tokio::time::timeout(Duration::from_secs(1), work)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    assert!(dropped.load(Ordering::Acquire));
    mailbox.wait_stopped().await.unwrap();
}
