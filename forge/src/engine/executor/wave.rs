//! Wave-based parallel lane planning.
//!
//! A "wave" is one scheduling round: the driver collects all READY tasks,
//! groups them into lanes, detects surface conflicts, and packs compatible
//! lanes into batches up to the concurrency cap.

use std::collections::BTreeMap;

use crate::engine::path::{file_of, overlap};

/// A lane of work with its associated surface paths.
/// Used by `plan_wave` to detect conflicts and pack batches.
#[derive(Debug, Clone)]
pub struct WaveLane<T> {
    pub lane: String,
    pub surface: Option<Vec<String>>,
    pub fanout: bool,
    pub task: T,
}

/// A refusal: two lanes whose surfaces overlap and cannot run in the same batch.
#[derive(Debug, Clone)]
pub struct WaveRefusal {
    pub lanes: (String, String),
    pub path: String,
}

/// The result of `plan_wave`: batches of compatible lanes and refusals.
#[derive(Debug, Clone)]
pub struct WavePlan<T> {
    pub batches: Vec<Vec<WaveLane<T>>>,
    pub refusals: Vec<WaveRefusal>,
}

/// Check if a lane has a non-empty surface.
fn has_surface<T>(lane: &WaveLane<T>) -> bool {
    lane.surface
        .as_ref()
        .map(|s| !s.is_empty())
        .unwrap_or(false)
}

/// Trie node for efficient surface path overlap detection.
/// Each node represents a path component and tracks which lane owns it.
#[derive(Default)]
struct SurfaceTrie {
    children: BTreeMap<String, SurfaceTrie>,
    owner: Option<String>,
}

impl SurfaceTrie {
    fn new() -> Self {
        Self {
            children: BTreeMap::new(),
            owner: None,
        }
    }

    /// Insert a path into the trie, returning any conflicting lane.
    /// A conflict occurs if the path overlaps with a path from a different lane.
    fn insert(&mut self, path: &str, lane: &str) -> Option<String> {
        let file = file_of(path)?;
        let components: Vec<&str> = file.split('/').collect();
        let mut node = self;
        for comp in components {
            node = node.children.entry(comp.to_string()).or_default();
            if let Some(owner) = &node.owner {
                if owner != lane {
                    // Found overlap with different lane
                    return Some(owner.clone());
                }
            } else {
                node.owner = Some(lane.to_string());
            }
        }
        None
    }

    /// Check if a path conflicts with any path in the trie from a different lane.
    fn check_conflict(&self, path: &str, lane: &str) -> Option<String> {
        let file = file_of(path)?;
        let components: Vec<&str> = file.split('/').collect();
        let mut node = self;
        for comp in components {
            if let Some(child) = node.children.get(comp) {
                node = child;
                if let Some(owner) = &node.owner {
                    if owner != lane {
                        return Some(owner.clone());
                    }
                }
            } else {
                return None;
            }
        }
        None
    }
}

/// Plan one scheduling wave: pack lanes into conflict-free batches up to `cap`.
///
/// Phase 1: Detect all pairwise conflicts using a trie (O(total_paths) instead of O(n²)).
/// Phase 2: Greedily pack lanes into batches respecting conflicts and capacity.
pub fn plan_wave<T: Clone>(lanes: &[WaveLane<T>], cap: usize) -> WavePlan<T> {
    let limit = cap.max(1);
    let mut refusals = Vec::new();

    // Phase 1: Detect all pairwise conflicts using a trie
    let mut trie = SurfaceTrie::new();
    let mut seen_conflicts = std::collections::BTreeSet::new();

    for lane in lanes {
        if lane.fanout || !has_surface(lane) {
            continue;
        }
        if let Some(surfaces) = &lane.surface {
            for path in surfaces {
                if let Some(conflicting_lane) = trie.check_conflict(path, &lane.lane) {
                    // Record conflict pair in canonical order to avoid duplicates
                    let pair = if lane.lane < conflicting_lane {
                        (lane.lane.clone(), conflicting_lane)
                    } else {
                        (conflicting_lane, lane.lane.clone())
                    };
                    if seen_conflicts.insert(pair.clone()) {
                        refusals.push(WaveRefusal {
                            lanes: pair,
                            path: path.clone(),
                        });
                    }
                }
            }
            // Insert this lane's paths into the trie for future lanes
            for path in surfaces {
                let _ = trie.insert(path, &lane.lane);
            }
        }
    }

    // Phase 2: Pack lanes into batches respecting conflicts and capacity
    let mut batches: Vec<Vec<WaveLane<T>>> = Vec::new();
    for lane in lanes {
        if !lane.fanout && !has_surface(lane) {
            batches.push(vec![lane.clone()]);
            continue;
        }
        let mut placed = false;
        for batch in batches.iter_mut() {
            if batch.len() >= limit {
                continue;
            }
            let compatible = batch.iter().all(|member| {
                if lane.fanout {
                    return member.fanout;
                }
                if member.fanout || !has_surface(member) {
                    return false;
                }
                // Check conflict using trie - build a temporary trie for this batch
                let mut batch_trie = SurfaceTrie::new();
                for m in batch.iter() {
                    if let Some(surfaces) = &m.surface {
                        for p in surfaces {
                            let _ = batch_trie.insert(p, &m.lane);
                        }
                    }
                }
                if let Some(surfaces) = &lane.surface {
                    for p in surfaces {
                        if batch_trie.check_conflict(p, &lane.lane).is_some() {
                            return false;
                        }
                    }
                }
                true
            });
            if !compatible {
                continue;
            }
            batch.push(lane.clone());
            placed = true;
            break;
        }
        if !placed {
            batches.push(vec![lane.clone()]);
        }
    }
    WavePlan { batches, refusals }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::fmt::Debug;

    fn path_component() -> impl Strategy<Value = String> {
        "[a-z]{1,8}".prop_map(String::from)
    }

    fn file_path() -> impl Strategy<Value = String> {
        prop::collection::vec(path_component(), 1..5)
            .prop_map(|parts| parts.join("/"))
    }

    fn wave_lane<T: Clone + Debug + 'static>(value: T) -> impl Strategy<Value = WaveLane<T>> {
        (prop::collection::vec(file_path(), 0..5), "[a-z]{1,10}", any::<bool>())
            .prop_map(move |(surface, lane, fanout)| WaveLane {
                surface: if surface.is_empty() { None } else { Some(surface) },
                lane,
                fanout,
                task: value.clone(),
            })
    }

    proptest! {
        #[test]
        fn no_conflicts_in_batches(
            lanes in prop::collection::vec(wave_lane(0u32), 1..20),
            cap in 1..5usize,
        ) {
            let plan = plan_wave(&lanes, cap);
            for batch in &plan.batches {
                for i in 0..batch.len() {
                    for j in i+1..batch.len() {
                        let lane_i = &batch[i];
                        let lane_j = &batch[j];
                        if lane_i.fanout || lane_j.fanout {
                            continue;
                        }
                        let surf_i = lane_i.surface.as_deref().unwrap_or(&[]);
                        let surf_j = lane_j.surface.as_deref().unwrap_or(&[]);
                        for a in surf_i {
                            for b in surf_j {
                                if overlap(a, b) {
                                    prop_assert!(false, "Conflicting surfaces in same batch: {} vs {}", a, b);
                                }
                            }
                        }
                    }
                }
            }
        }

        #[test]
        fn all_lanes_placed(
            lanes in prop::collection::vec(wave_lane(0u32), 1..20),
            cap in 1..5usize,
        ) {
            let plan = plan_wave(&lanes, cap);
            let placed: Vec<_> = plan.batches.iter().flatten().collect();
            let refused: Vec<_> = plan.refusals.iter().flat_map(|r| [r.lanes.0.clone(), r.lanes.1.clone()]).collect();
            for lane in &lanes {
                let is_placed = placed.iter().any(|l| l.lane == lane.lane);
                let is_refused = refused.contains(&lane.lane);
                if lane.surface.is_none() || !lane.surface.as_deref().map_or(false, |s| !s.is_empty()) || lane.fanout {
                    prop_assert!(is_placed, "Lane {} should be placed", lane.lane);
                } else {
                    prop_assert!(is_placed || is_refused, "Lane {} should be placed or refused", lane.lane);
                }
            }
        }

        #[test]
        fn batch_size_respects_cap(
            lanes in prop::collection::vec(wave_lane(0u32), 1..20),
            cap in 1..5usize,
        ) {
            let plan = plan_wave(&lanes, cap);
            for batch in &plan.batches {
                prop_assert!(batch.len() <= cap, "Batch size {} exceeds cap {}", batch.len(), cap);
            }
        }

        #[test]
        fn refusals_are_symmetric(
            lanes in prop::collection::vec(wave_lane(0u32), 1..20),
            cap in 2..5usize,
        ) {
            let plan = plan_wave(&lanes, cap);
            for r in &plan.refusals {
                prop_assert!(!r.path.is_empty(), "Refusal path should not be empty");
                let lane_a = lanes.iter().find(|l| l.lane == r.lanes.0);
                let lane_b = lanes.iter().find(|l| l.lane == r.lanes.1);
                if let (Some(a), Some(b)) = (lane_a, lane_b) {
                    let surf_a = a.surface.as_deref().unwrap_or(&[]);
                    let surf_b = b.surface.as_deref().unwrap_or(&[]);
                    let path_from_a = surf_a.iter().any(|x| x == &r.path);
                    let path_from_b = surf_b.iter().any(|x| x == &r.path);
                    prop_assert!(path_from_a || path_from_b, "Refusal path {} should come from one of the conflicting lanes", r.path);
                }
            }
        }
    }
}