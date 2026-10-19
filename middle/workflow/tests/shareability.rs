//! Adapter-side half of the old `concurrency::engine_and_stores_are_shareable_across_threads`
//! test: the Neon adapter lives in this crate, so its `Send + Sync` proof stays with it.
//! The memory-store half now lives in `workflow-core`.

#[test]
fn neon_store_and_engine_are_shareable_across_threads() {
    fn assert_shareable<T: Send + Sync>() {}
    assert_shareable::<workflow::NeonStore>();
    assert_shareable::<workflow::WorkflowEngine<workflow::NeonStore>>();
}
