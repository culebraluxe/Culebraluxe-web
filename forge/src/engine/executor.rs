//! The executor: who drives a story's roles, what a wave is, and how a stop cap resolves.
//!
//! `executor.rs` was one file until 2026-10-04. The split into the five submodules below is an implementation
//! detail, so the flat surface this module had as one file is re-exported here and callers keep naming the
//! executor — `forge::engine::executor::ForgeRoleRunner` — instead of its file layout. Keeping the module root a
//! file beside its directory (rather than an inline `mod` inside `engine/mod.rs`) is deliberate: the contract
//! suite, the CI boundedness baseline and this crate's own tests all name `forge/src/engine/executor.rs`, and an
//! inline module deletes that path.

pub mod completion;
pub mod dispatch;
pub mod drive;
pub mod lane_failure;
pub mod wave;

pub use self::dispatch::{parse_forge_stop_after, resolve_forge_stop_target, ForgeStopTarget};
pub use self::drive::{
    drive_forge_story, drive_forge_story_with_jobs, DriveForgeStoryOptions, DriveForgeStoryResult,
    DurableForgeExecution, ForgeRoleOutcome, ForgeRoleRunner,
};
pub use self::lane_failure::{
    is_advance_conflict, is_completed_release_conflict, settle_forge_lane_failure,
    LaneFailureSettlement,
};
pub use self::wave::{plan_wave, WaveLane, WavePlan, WaveRefusal};
