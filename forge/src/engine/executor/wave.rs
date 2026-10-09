//! Wave-based parallel lane planning.
//!
//! A "wave" is one scheduling round: the driver collects all READY tasks,
//! groups them into lanes, detects surface conflicts, and packs compatible
//! lanes into batches up to the concurrency cap.

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
        .map(|s| !s.is_empty() && s.iter().all(|path| canonical_surface(path).is_some()))
        .unwrap_or(false)
}

/// Surface syntax is explicit: `dir/` declares a tree, while `file` (including
/// extensionless names) declares one exact path. Invalid and unsupported paths
/// are unknown, so their lane runs alone.
fn canonical_surface(raw: &str) -> Option<(String, bool)> {
    let raw = raw.trim();
    if raw.is_empty()
        || raw.starts_with('/')
        || raw
            .chars()
            .any(|ch| matches!(ch, '*' | '?' | '[' | ']' | '{' | '}' | '\\'))
        || raw.chars().any(char::is_control)
    {
        return None;
    }
    let tree = raw.ends_with('/');
    let mut parts = Vec::new();
    for part in raw.trim_start_matches("./").split('/') {
        match part {
            "" | "." => continue,
            ".." => return None,
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some((parts.join("/"), tree))
    }
}

fn surfaces_conflict(left: &str, right: &str) -> bool {
    let (Some((left, left_tree)), Some((right, right_tree))) =
        (canonical_surface(left), canonical_surface(right))
    else {
        return true;
    };
    left == right
        || (left_tree && right.starts_with(&format!("{left}/")))
        || (right_tree && left.starts_with(&format!("{right}/")))
}

/// Whether a repository path is contained by one declared surface. A trailing slash
/// on a declaration is the explicit tree marker; an exact declaration owns one file.
pub fn surface_contains(surface: &[String], changed_path: &str) -> bool {
    let Some((changed, _)) = canonical_surface(changed_path) else {
        return false;
    };
    surface.iter().any(|declared| {
        canonical_surface(declared).is_some_and(|(path, tree)| {
            path == changed || (tree && changed.starts_with(&format!("{path}/")))
        })
    })
}

fn lane_conflict<T>(left: &WaveLane<T>, right: &WaveLane<T>) -> Option<String> {
    let (Some(left_paths), Some(right_paths)) = (&left.surface, &right.surface) else {
        return None;
    };
    if left_paths.is_empty() || right_paths.is_empty() {
        return None;
    }
    for a in left_paths {
        for b in right_paths {
            if surfaces_conflict(a, b) {
                return Some(match (canonical_surface(a), canonical_surface(b)) {
                    (None, _) => a.clone(),
                    (_, None) => b.clone(),
                    (Some((left, left_tree)), Some((right, right_tree))) => {
                        if left == right {
                            left
                        } else if left_tree {
                            right
                        } else if right_tree {
                            left
                        } else {
                            right
                        }
                    }
                });
            }
        }
    }
    None
}

/// Plan one scheduling wave: pack lanes into conflict-free batches up to `cap`.
///
/// Plan with a pairwise oracle. Lane counts are deliberately small; keeping
/// path semantics obvious is safer than a trie that conflates directory
/// prefixes with file ownership.
pub fn plan_wave<T: Clone>(lanes: &[WaveLane<T>], cap: usize) -> WavePlan<T> {
    let limit = cap.max(1);
    let mut refusals = Vec::new();
    for i in 0..lanes.len() {
        for j in (i + 1)..lanes.len() {
            if let Some(path) = lane_conflict(&lanes[i], &lanes[j]) {
                refusals.push(WaveRefusal {
                    lanes: (lanes[i].lane.clone(), lanes[j].lane.clone()),
                    path,
                });
            }
        }
    }

    let mut batches: Vec<Vec<WaveLane<T>>> = Vec::new();
    for lane in lanes {
        if !has_surface(lane) {
            batches.push(vec![lane.clone()]);
            continue;
        }
        if let Some(batch) = batches.iter_mut().find(|batch| {
            batch.len() < limit
                && batch
                    .iter()
                    .all(|member| has_surface(member) && lane_conflict(member, lane).is_none())
        }) {
            batch.push(lane.clone());
        } else {
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
        prop::collection::vec(path_component(), 1..5).prop_map(|parts| parts.join("/"))
    }

    fn surface_syntax() -> impl Strategy<Value = String> {
        prop_oneof![
            file_path(),
            file_path().prop_map(|path| format!("././{path}//")),
            file_path().prop_map(|path| format!("{path}/./leaf")),
            Just("../escape".to_string()),
            Just("/absolute/path".to_string()),
            Just("src/**".to_string()),
            Just("src/[ab].rs".to_string()),
            Just("".to_string()),
        ]
    }

    fn canonical_oracle(raw: &str) -> Option<(String, bool)> {
        let value = raw.trim().trim_start_matches("./");
        if value.is_empty()
            || value.starts_with('/')
            || value
                .chars()
                .any(|ch| matches!(ch, '*' | '?' | '[' | ']' | '{' | '}' | '\\') || ch.is_control())
        {
            return None;
        }
        let tree = value.ends_with('/');
        let mut normalized = Vec::new();
        for part in value.split('/') {
            match part {
                "" | "." => {}
                ".." => return None,
                segment => normalized.push(segment),
            }
        }
        (!normalized.is_empty()).then(|| (normalized.join("/"), tree))
    }

    fn oracle_conflict(left: &str, right: &str) -> Option<String> {
        let a = canonical_oracle(left);
        let b = canonical_oracle(right);
        match (a, b) {
            (None, _) => Some(left.to_string()),
            (_, None) => Some(right.to_string()),
            (Some((a, a_tree)), Some((b, b_tree)))
                if a == b
                    || (a_tree && b.starts_with(&format!("{a}/")))
                    || (b_tree && a.starts_with(&format!("{b}/"))) =>
            {
                Some(if a == b || a_tree {
                    left.to_string()
                } else {
                    right.to_string()
                })
            }
            _ => None,
        }
    }

    fn wave_lane<T: Clone + Debug + 'static>(value: T) -> impl Strategy<Value = WaveLane<T>> {
        (
            prop::collection::vec(file_path(), 0..5),
            "[a-z]{1,10}",
            any::<bool>(),
        )
            .prop_map(move |(surface, lane, fanout)| WaveLane {
                surface: if surface.is_empty() {
                    None
                } else {
                    Some(surface)
                },
                lane,
                fanout,
                task: value.clone(),
            })
    }

    /// Lane names are unique in production (`lane/<name>`), so the strategy must not hand two lanes one name: a
    /// refusal names its conflicting lanes by name, and with duplicates that lookup picks the *first* lane of that
    /// name — which may be a lane that holds no such path at all (found by proptest on 2026-10-04: lanes e/p,
    /// d/none, d/p_a made `refusals_are_symmetric` fail on a refusal that was correct). The same ambiguity made
    /// `all_lanes_placed` pass for the wrong reason. Every property below takes its lanes through here.
    fn unique_lanes<T: Clone + Debug + 'static>(lanes: Vec<WaveLane<T>>) -> Vec<WaveLane<T>> {
        lanes
            .into_iter()
            .enumerate()
            .map(|(index, mut lane)| {
                lane.lane = format!("{}{index}", lane.lane);
                lane
            })
            .collect()
    }

    proptest! {
        #[test]
        fn no_conflicts_in_batches(
            lanes in prop::collection::vec(wave_lane(0u32), 1..20).prop_map(|lanes| unique_lanes(lanes)),
            cap in 1..5usize,
        ) {
            let plan = plan_wave(&lanes, cap);
            for batch in &plan.batches {
                for i in 0..batch.len() {
                    for j in i+1..batch.len() {
                        let lane_i = &batch[i];
                        let lane_j = &batch[j];
                        let surf_i = lane_i.surface.as_deref().unwrap_or(&[]);
                        let surf_j = lane_j.surface.as_deref().unwrap_or(&[]);
                        for a in surf_i {
                            for b in surf_j {
                                if surfaces_conflict(a, b) {
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
            lanes in prop::collection::vec(wave_lane(0u32), 1..20).prop_map(|lanes| unique_lanes(lanes)),
            cap in 1..5usize,
        ) {
            let plan = plan_wave(&lanes, cap);
            let placed: Vec<_> = plan.batches.iter().flatten().collect();
            let refused: Vec<_> = plan.refusals.iter().flat_map(|r| [r.lanes.0.clone(), r.lanes.1.clone()]).collect();
            for lane in &lanes {
                let is_placed = placed.iter().any(|l| l.lane == lane.lane);
                let is_refused = refused.contains(&lane.lane);
                if lane.surface.is_none() || !lane.surface.as_deref().map_or(false, |s| !s.is_empty()) {
                    prop_assert!(is_placed, "Lane {} should be placed", lane.lane);
                } else {
                    prop_assert!(is_placed || is_refused, "Lane {} should be placed or refused", lane.lane);
                }
            }
        }

        #[test]
        fn batch_size_respects_cap(
            lanes in prop::collection::vec(wave_lane(0u32), 1..20).prop_map(|lanes| unique_lanes(lanes)),
            cap in 1..5usize,
        ) {
            let plan = plan_wave(&lanes, cap);
            for batch in &plan.batches {
                prop_assert!(batch.len() <= cap, "Batch size {} exceeds cap {}", batch.len(), cap);
            }
        }

        #[test]
        fn refusals_are_symmetric(
            lanes in prop::collection::vec(wave_lane(0u32), 1..20).prop_map(|lanes| unique_lanes(lanes)),
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

    #[test]
    fn sibling_files_do_not_conflict_but_explicit_tree_prefix_does() {
        let lane = |id: &str, path: &str| WaveLane {
            lane: id.into(),
            surface: Some(vec![path.into()]),
            fanout: false,
            task: (),
        };
        assert_eq!(
            plan_wave(&[lane("a", "src/a.rs"), lane("b", "src/b.rs")], 2).batches[0].len(),
            2
        );
        assert_eq!(
            plan_wave(&[lane("a", "src/a.rs"), lane("b", "src/a.rs")], 2).batches[0].len(),
            1
        );
        assert_eq!(
            plan_wave(&[lane("a", "src/"), lane("b", "src/a.rs")], 2).batches[0].len(),
            1
        );
        assert_eq!(
            plan_wave(&[lane("a", "src/a"), lane("b", "src/ab")], 2).batches[0].len(),
            2
        );
    }

    #[test]
    fn surface_containment_uses_exact_files_and_explicit_tree_prefixes() {
        assert!(surface_contains(&["src/a.rs".into()], "src/a.rs"));
        assert!(!surface_contains(&["src/a.rs".into()], "src/a/b.rs"));
        assert!(surface_contains(&["src/".into()], "src/a/b.rs"));
        assert!(!surface_contains(&["src/".into()], "src-old/a.rs"));
        assert!(surface_contains(&["./src//".into()], "src/a.rs"));
    }

    proptest! {
        #[test]
        fn two_lane_decisions_match_independent_pairwise_oracle(
            a in surface_syntax(),
            b in surface_syntax(),
            a_tree in any::<bool>(),
            b_tree in any::<bool>(),
        ) {
            let a = if a_tree { format!("{a}/") } else { a };
            let b = if b_tree { format!("{b}/") } else { b };
            let lanes = [
                WaveLane { lane: "a".into(), surface: Some(vec![a.clone()]), fanout: false, task: 1 },
                WaveLane { lane: "b".into(), surface: Some(vec![b.clone()]), fanout: false, task: 2 },
            ];
            let plan = plan_wave(&lanes, 2);
            let share_batch = plan.batches.iter().any(|batch| batch.len() == 2);
            prop_assert_eq!(share_batch, oracle_conflict(&a, &b).is_none(), "{:?} versus {:?}", a, b);
        }
    }

    #[test]
    fn unknown_and_invalid_surfaces_are_serialized_even_when_fanout_is_set() {
        let lanes = vec![
            WaveLane {
                lane: "known".into(),
                surface: Some(vec!["src/a.rs".into()]),
                fanout: true,
                task: 1,
            },
            WaveLane {
                lane: "unknown".into(),
                surface: None,
                fanout: true,
                task: 2,
            },
            WaveLane {
                lane: "invalid".into(),
                surface: Some(vec!["../escape".into()]),
                fanout: true,
                task: 3,
            },
        ];
        let plan = plan_wave(&lanes, 4);
        assert_eq!(plan.batches.len(), 3);
        assert!(plan.batches.iter().all(|batch| batch.len() == 1));
        assert!(canonical_surface("src/**").is_none());
        assert!(canonical_surface("src/[ab].rs").is_none());
    }
}
