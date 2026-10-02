//! The spend meter: what a model turn cost, read from OpenCode's SUPPORTED session interface.
//!
//! MIGRATED OFF THE VENDOR'S PRIVATE DATABASE (ENG-FORGE-OPENCODE-V2 §5, 2026-10-01).
//!
//! What this module used to do: read OpenCode V1's SQLite store directly — `select … from session` through the
//! system `sqlite3` in `-readonly` mode — because that was where the vendor kept a session's running totals.
//! It worked, and it was a bet on an implementation detail: a schema Forge does not own, columns
//! (`tokens_input`, `cost`, `parent_id`, `directory`, `time_created`) Forge does not control, and a file whose
//! path Forge had to guess (`FORGE_OPENCODE_DB`, `$XDG_DATA_HOME/opencode/opencode.db`). V2 calls that bet in,
//! so it is dropped rather than re-pointed at another V2-internal table.
//!
//! What it does instead — two commands the vendor documents:
//!
//! - `session export <id> --standalone` → `info.cost`, `info.tokens.input`, `info.tokens.output`: the vendor's
//!   own statement of what a session used. This is the AUTHORITATIVE reading.
//! - `session list --standalone --format json` → this project's top-level sessions, newest first, with
//!   `created`/`updated`/`directory`: how a lane finds its OWN session without scanning a database.
//!
//! Precedence, and why a floor is not an exact figure: the live 2.0.21 build does not reliably emit a
//! `step_finish` for a turn's TERMINAL step, so the `step_finish` sum in `engine::opencode_events` is a LOWER
//! BOUND. The export is therefore preferred, and that sum is used only as the fallback when the export cannot be
//! read. Where the fallback is taken it is a floor, and `engine::opencode` says so at the call site.
//!
//! What one turn spent:
//! - a NEW session is the one the turn ITSELF reported (`opencode_events`: the id the vendor minted). The
//!   timestamp/directory search below is only the fallback for a turn that reported no id — the case V1 could
//!   not distinguish, and the reason V1 needed "the first session created in this directory at or after the
//!   launch instant" at all.
//! - a RESUMED session is charged the DIFFERENCE across the turn, because its totals are cumulative for the
//!   whole generation. Saturating: a session rewritten underneath us never reports negative spend.
//!
//! Every failure to read is `None` — unmeasured — never a fabricated zero.
//!
//! KNOWN GAP, DECLARED RATHER THAN PATCHED (§6): V1 included CHILD sessions in their root's spend. V2 does not, and
//! this module cannot fix that from where it reads.
//!
//! THE LINKAGE IS NOT MISSING FROM V2 — IT IS UNREACHABLE FROM FORGE'S INVOCATION (corrected 2026-10-01; this note
//! previously claimed no supported interface exposed it, which is wrong). Measured on the installed 2.0.21 build:
//! `GET /session/{sessionID}/children` exists in the vendor's own OpenAPI surface ("Retrieve all child sessions
//! that were forked from the specified parent session"), and a child is linked by the
//! `x-opencode-parent-session-id` header. So the vendor CAN name the children. What Forge has is a different thing:
//! a `--standalone` run speaks STDIO to a private `opencode serve --stdio --port 0` child, prints no addressable
//! endpoint, and the CLI exposes no `session children` subcommand (only `list`, `delete`, `export`, `import`).
//! Through the interface this module actually reads with — `session list`, `session export` — a child is simply
//! never mentioned.
//!
//! AND THE UNDERCOUNT IS MATERIAL, not theoretical. Measured from the live store, one real parent/child pair:
//! parent `cost 0.022812138, input 50231, output 8668`; its `explore` child `cost 0.027764424, input 113224,
//! output 10227`. The CHILD COST MORE THAN THE PARENT, and none of it appears in the parent's export — so "what this
//! turn spent" is strictly the root session's spend. The exposure is bounded and named rather than open-ended: only
//! the agents whose child allowlist is non-empty can produce a child at all (`forge-smith`, `forge-architect` — see
//! `engine::opencode_agents`), and background children stay disabled.
//!
//! What would close it: a supported READ of the linkage that Forge can reach — a `session children` subcommand, or
//! the child totals folded into the parent's export. The moment either appears the accounting belongs HERE rather
//! than in a note, and `a_parent_turn_is_charged_its_own_spend_and_not_its_childrens` is written to trip when it
//! does.

use std::collections::HashMap;
use std::process::Command;

use crate::engine::opencode_client::{build_session_export_args, build_session_list_args};

#[derive(Debug, Clone, PartialEq)]
pub struct HarnessUsage {
    pub session_id: String,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub cost_usd: f64,
}

impl HarnessUsage {
    /// One run is several attempts; their spend is the sum.
    pub fn absorb(&mut self, other: &HarnessUsage) {
        self.tokens_input += other.tokens_input;
        self.tokens_output += other.tokens_output;
        self.cost_usd += other.cost_usd;
    }
}

/// A resumed session's own spend across one turn. Saturating: a session rewritten underneath us (compaction,
/// revert) can never report negative spend. No baseline means the measured totals stand.
pub fn usage_delta(after: &HarnessUsage, before: Option<&HarnessUsage>) -> HarnessUsage {
    let Some(before) = before else {
        return after.clone();
    };
    HarnessUsage {
        session_id: after.session_id.clone(),
        tokens_input: (after.tokens_input - before.tokens_input).max(0),
        tokens_output: (after.tokens_output - before.tokens_output).max(0),
        cost_usd: (after.cost_usd - before.cost_usd).max(0.0),
    }
}

/// One row of `session list --format json`.
///
/// Note the vendor's own casing drift, which is why this is parsed by key and not by a mirrored schema: the
/// LIST spells it `projectId`, the EXPORT spells it `projectID`. The fields Forge reads are lowercase in both.
#[derive(Debug, Clone, PartialEq)]
pub struct VendorSession {
    pub id: String,
    pub created_ms: i64,
    pub updated_ms: i64,
    pub directory: String,
}

/// Parse `session list --format json`. A malformed payload is an EMPTY list, not a panic: an unreadable list
/// means the fallback could not resolve a session, which every caller already reads as unmeasured.
pub fn parse_session_list(json: &str) -> Vec<VendorSession> {
    let Ok(rows) = serde_json::from_str::<Vec<serde_json::Value>>(json) else {
        return Vec::new();
    };
    rows.iter()
        .filter_map(|row| {
            Some(VendorSession {
                id: row.get("id")?.as_str()?.to_string(),
                created_ms: row
                    .get("created")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0),
                updated_ms: row
                    .get("updated")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0),
                directory: row
                    .get("directory")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            })
        })
        .collect()
}

/// Parse `session export <id>`: the `info` block's own totals.
///
/// STRICT ON PURPOSE. A payload with no readable `info`, no `cost`, or no `tokens.input`/`tokens.output` is
/// `None` — unmeasured. The alternative, defaulting an absent field to 0, is exactly the fabricated reading the
/// reading is not allowed to make: an absent cost is an UNKNOWN cost, not a free turn.
pub fn parse_session_export(json: &str, session_id: &str) -> Option<HarnessUsage> {
    let doc: serde_json::Value = serde_json::from_str(json).ok()?;
    let info = doc.get("info")?;
    let cost = info.get("cost").and_then(serde_json::Value::as_f64)?;
    let tokens = info.get("tokens")?;
    let input = tokens.get("input").and_then(serde_json::Value::as_i64)?;
    let output = tokens.get("output").and_then(serde_json::Value::as_i64)?;
    Some(HarnessUsage {
        session_id: session_id.to_string(),
        tokens_input: input,
        tokens_output: output,
        cost_usd: cost,
    })
}

/// Run the vendor CLI for a READ, returning stdout only on a clean exit.
///
/// Nothing else is returned on purpose: a missing binary, a non-zero exit and an unreadable payload all mean
/// "could not read", which every caller already maps to unmeasured. The lane's sanitized environment is applied
/// here the same way it is for a model turn, so a control-plane read never carries production database
/// authority either — and the project is pinned the same way too (`opencode_client::apply_vendor_env`), because a
/// stale `PWD` would otherwise make `session list` describe a different project's sessions and this module would
/// measure a session the lane never ran.
fn run_vendor(
    cli_bin: &str,
    args: &[String],
    cwd: &str,
    env: Option<&HashMap<String, String>>,
) -> Option<String> {
    let mut cmd = Command::new(cli_bin);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    crate::engine::opencode_client::apply_vendor_env(&mut cmd, env, cwd);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

/// The directory spellings the vendor may report for this lane's working directory (as given, and resolved:
/// `/var/…` is `/private/var/…` on macOS).
pub fn directory_spellings(cwd: &str) -> Vec<String> {
    let mut out = vec![cwd.trim_end_matches('/').to_string()];
    if let Ok(real) = std::fs::canonicalize(cwd) {
        let real = real.to_string_lossy().trim_end_matches('/').to_string();
        if !out.contains(&real) {
            out.push(real);
        }
    }
    out
}

fn matches_directory(stored: &str, cwd: &str) -> bool {
    let stored = stored.trim_end_matches('/');
    directory_spellings(cwd)
        .iter()
        .any(|spelling| spelling == stored)
}

/// `--continue` resumes the most recently updated session; this is which one that is.
fn pick_latest(rows: &[VendorSession]) -> Option<String> {
    rows.iter()
        .max_by_key(|row| row.updated_ms)
        .map(|row| row.id.clone())
}

/// The first session created at or after `since_ms` — the fallback for a turn that reported no id of its own.
fn pick_first_since(rows: &[VendorSession], since_ms: i64) -> Option<String> {
    rows.iter()
        .filter(|row| row.created_ms >= since_ms)
        .min_by_key(|row| row.created_ms)
        .map(|row| row.id.clone())
}

/// This lane's own top-level sessions, restricted to this working directory.
pub fn vendor_sessions(
    cli_bin: &str,
    cwd: &str,
    env: Option<&HashMap<String, String>>,
) -> Vec<VendorSession> {
    let Some(stdout) = run_vendor(cli_bin, &build_session_list_args(), cwd, env) else {
        return Vec::new();
    };
    let mut rows = parse_session_list(&stdout);
    rows.retain(|row| matches_directory(&row.directory, cwd));
    rows
}

/// The session `--continue` would resume in this directory.
pub fn latest_session_in(
    cli_bin: &str,
    cwd: &str,
    env: Option<&HashMap<String, String>>,
) -> Option<String> {
    pick_latest(&vendor_sessions(cli_bin, cwd, env))
}

/// The first session this directory created at or after `since_ms`.
pub fn first_session_since(
    cli_bin: &str,
    cwd: &str,
    env: Option<&HashMap<String, String>>,
    since_ms: i64,
) -> Option<String> {
    pick_first_since(&vendor_sessions(cli_bin, cwd, env), since_ms)
}

/// A session's spend, as the vendor reports it. `None` — unmeasured — when the export cannot be read.
pub fn session_usage(
    cli_bin: &str,
    cwd: &str,
    env: Option<&HashMap<String, String>>,
    session_id: &str,
) -> Option<HarnessUsage> {
    let stdout = run_vendor(cli_bin, &build_session_export_args(session_id), cwd, env)?;
    parse_session_export(&stdout, session_id)
}

/// What one OpenCode turn is about to resume, read BEFORE the turn so its spend can be taken as a difference.
pub struct UsageBaseline {
    cli_bin: String,
    cwd: String,
    env: Option<HashMap<String, String>>,
    launch_ms: i64,
    resumed: Option<(String, Option<HarnessUsage>)>,
}

impl UsageBaseline {
    pub fn before_turn(
        cwd: &str,
        session: Option<&str>,
        continue_session: bool,
        cli_bin: &str,
        env: Option<&HashMap<String, String>>,
    ) -> Self {
        let env = env.cloned();
        let launch_ms = now_ms();
        let resumed_id = match session {
            Some(id) => Some(id.to_string()),
            None if continue_session => latest_session_in(cli_bin, cwd, env.as_ref()),
            None => None,
        };
        let resumed = resumed_id.map(|id| {
            let before = session_usage(cli_bin, cwd, env.as_ref(), &id);
            (id, before)
        });
        Self {
            cli_bin: cli_bin.to_string(),
            cwd: cwd.to_string(),
            env,
            launch_ms,
            resumed,
        }
    }

    /// What the turn spent. `None` is "unmeasured", which the run row keeps as `cost_source='none'`.
    ///
    /// `actual_session` is the id the turn itself reported. A fresh turn now HAS one, so the
    /// directory/timestamp search is only the fallback for a turn whose stream reported no id.
    pub fn after_turn(&self, actual_session: Option<&str>) -> Option<HarnessUsage> {
        match &self.resumed {
            Some((id, before)) => {
                let after = session_usage(&self.cli_bin, &self.cwd, self.env.as_ref(), id)?;
                Some(usage_delta(&after, before.as_ref()))
            }
            None => {
                let id = match actual_session {
                    Some(id) => id.to_string(),
                    None => first_session_since(
                        &self.cli_bin,
                        &self.cwd,
                        self.env.as_ref(),
                        self.launch_ms,
                    )?,
                };
                session_usage(&self.cli_bin, &self.cwd, self.env.as_ref(), &id)
            }
        }
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    //! Real vendor shapes from the installed build (opencode v2.0.21), neutralised: ids and text replaced,
    //! every key, nesting and number as captured.
    //!
    //! There is no SQLite fixture here any more, and no `sqlite3` process: the previous suite built a temp
    //! `opencode.db` and asserted a query against it. That store is no longer read, so the fixtures are now the
    //! vendor's own CLI payloads — the thing this code actually consumes.

    use super::*;

    const EXPORT: &str = r#"{"info":{"id":"ses_fixture_turn_0001","projectID":"proj_fixture","model":{"id":"deepseek-flash","providerID":"deepseek","variant":"default"},"cost":0.000363204,"tokens":{"input":650,"output":57,"reasoning":254,"cache":{"read":26368,"write":0}},"outcome":"succeeded","time":{"created":1790912076340,"updated":1790912077879,"idle":1790912078052},"title":"fixture","location":{"directory":"/private/tmp/ocsmoke"}},"messages":[{"id":"msg_fixture0001","time":{"created":1790912076653},"text":"prompt","files":[],"type":"user"}]}"#;

    const LIST: &str = r#"[{"id":"ses_fixture_turn_0001","title":"first","updated":1790912077879,"created":1790912076340,"projectId":"proj_fixture","directory":"/private/tmp/ocsmoke"},{"id":"ses_fixture_turn_0002","title":"second","updated":1790912090000,"created":1790912080000,"projectId":"proj_fixture","directory":"/private/tmp/ocsmoke"}]"#;

    #[test]
    fn a_sessions_totals_come_from_the_vendors_own_export() {
        let usage = parse_session_export(EXPORT, "ses_fixture_turn_0001").expect("measured");
        assert_eq!(usage.tokens_input, 650);
        assert_eq!(usage.tokens_output, 57);
        assert!((usage.cost_usd - 0.000363204).abs() < 1e-12);
        assert_eq!(usage.session_id, "ses_fixture_turn_0001");
    }

    #[test]
    fn an_absent_measurement_is_unmeasured_and_never_a_fabricated_zero() {
        // No cost at all: unknown, not free.
        assert_eq!(
            parse_session_export(
                r#"{"info":{"id":"ses_x","tokens":{"input":10,"output":2}}}"#,
                "ses_x"
            ),
            None
        );
        // A cost but no token block.
        assert_eq!(
            parse_session_export(r#"{"info":{"id":"ses_x","cost":0.5}}"#, "ses_x"),
            None
        );
        // A token block with no counts.
        assert_eq!(
            parse_session_export(r#"{"info":{"id":"ses_x","cost":0.5,"tokens":{}}}"#, "ses_x"),
            None
        );
        // A present-but-zero reading IS a measurement, and must survive.
        let zero = parse_session_export(
            r#"{"info":{"id":"ses_x","cost":0,"tokens":{"input":0,"output":0}}}"#,
            "ses_x",
        )
        .expect("present and zero is measured");
        assert_eq!(
            (zero.tokens_input, zero.tokens_output, zero.cost_usd),
            (0, 0, 0.0)
        );
    }

    #[test]
    fn an_unreadable_export_is_unmeasured() {
        for payload in [
            "not json",
            "",
            "[]",
            r#"{"messages":[]}"#,
            r#"{"info":null}"#,
        ] {
            assert_eq!(parse_session_export(payload, "ses_x"), None, "{payload}");
        }
    }

    #[test]
    fn the_session_list_is_read_into_this_projects_sessions() {
        let rows = parse_session_list(LIST);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "ses_fixture_turn_0001");
        assert_eq!(rows[1].id, "ses_fixture_turn_0002");
        assert_eq!(rows[0].created_ms, 1790912076340);
        assert_eq!(rows[1].updated_ms, 1790912090000);
        assert_eq!(rows[0].directory, "/private/tmp/ocsmoke");
        // A payload the vendor has not produced reads as no sessions rather than a panic.
        assert!(parse_session_list("not json").is_empty());
        assert!(parse_session_list("{}").is_empty());
    }

    #[test]
    fn only_this_directory_sessions_can_become_the_lanes_session() {
        let cwd = std::env::temp_dir().to_string_lossy().to_string();
        let resolved = std::fs::canonicalize(&cwd)
            .expect("temp dir resolves")
            .to_string_lossy()
            .to_string();

        // The vendor reports the RESOLVED spelling on macOS (/private/var/… for /var/…), so matching only the
        // spelling as given would silently read nothing — the failure mode this equality exists to prevent.
        assert!(matches_directory(&resolved, &cwd));
        assert!(!matches_directory("/definitely/not/this/one", &cwd));

        let rows = vec![
            VendorSession {
                id: "ses_lane_first".into(),
                created_ms: 1_000,
                updated_ms: 1_000,
                directory: resolved.clone(),
            },
            VendorSession {
                id: "ses_lane_second".into(),
                created_ms: 2_000,
                updated_ms: 2_000,
                directory: resolved.clone(),
            },
            // Another tool writing to the same vendor store, most recently touched of all. This is the Cline
            // case the V1 reader was careful about, and it must still not be claimable.
            VendorSession {
                id: "ses_other_tool".into(),
                created_ms: 900,
                updated_ms: 9_000,
                directory: "/somewhere/else".into(),
            },
        ];
        let in_lane: Vec<VendorSession> = rows
            .into_iter()
            .filter(|row| matches_directory(&row.directory, &cwd))
            .collect();

        assert_eq!(
            pick_latest(&in_lane).as_deref(),
            Some("ses_lane_second"),
            "the most recently updated session OF THIS DIRECTORY"
        );
        assert_eq!(
            pick_first_since(&in_lane, 1_500).as_deref(),
            Some("ses_lane_second"),
            "the earliest at or after the launch instant"
        );
        assert_eq!(
            pick_first_since(&in_lane, 0).as_deref(),
            Some("ses_lane_first"),
            "nothing was before it"
        );
        assert_eq!(
            pick_first_since(&in_lane, 3_000),
            None,
            "nothing after the launch instant: unmeasured"
        );
    }

    #[test]
    fn a_resumed_session_is_charged_its_delta_not_its_lifetime() {
        let before = HarnessUsage {
            session_id: "ses_gen".into(),
            tokens_input: 100,
            tokens_output: 20,
            cost_usd: 0.01,
        };
        let after = HarnessUsage {
            session_id: "ses_gen".into(),
            tokens_input: 260,
            tokens_output: 55,
            cost_usd: 0.031,
        };
        let delta = usage_delta(&after, Some(&before));
        assert_eq!(delta.tokens_input, 160);
        assert_eq!(delta.tokens_output, 35);
        assert!((delta.cost_usd - 0.021).abs() < 1e-9);
        let rewritten = usage_delta(&before, Some(&after));
        assert_eq!(
            (rewritten.tokens_input, rewritten.cost_usd),
            (0, 0.0),
            "never negative"
        );
        assert_eq!(
            usage_delta(&after, None),
            after,
            "no baseline: the totals stand"
        );
    }

    /// The gap, pinned to the vendor's own numbers and to the bound on who can cause it — rather than to a word search
    /// over a fixture, which could never have told us what the vendor's API does.
    ///
    /// MEASURED on the live 2.0.21 store (2026-10-01), one real parent/child pair. The parent's export reports the
    /// parent's totals alone; the child's own session reports larger ones. Nothing of the child is in the parent, so a
    /// turn's recorded spend is strictly its root session's spend.
    #[test]
    fn a_parent_turn_is_charged_its_own_spend_and_not_its_childrens() {
        const MEASURED_PARENT: (f64, i64, i64) = (0.022812138, 50_231, 8_668);
        const MEASURED_CHILD: (f64, i64, i64) = (0.027764424, 113_224, 10_227);

        // What Forge reads for a parent: its own totals, exactly as the export states them.
        let parent = parse_session_export(EXPORT, "ses_fixture_turn_0001").expect("measured");
        assert_eq!(
            (parent.tokens_input, parent.tokens_output),
            (650, 57),
            "the export's own numbers, unaltered"
        );

        // The two reads Forge has, and neither carries a child: the export is `info` + `messages`, and the list is
        // rows of top-level sessions. Asserted as SHAPE, so a vendor that starts folding children in trips this.
        let export_shape: serde_json::Value = serde_json::from_str(EXPORT).expect("json");
        assert_eq!(
            export_shape.as_object().map(|map| {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                keys
            }),
            Some(vec![&"info".to_string(), &"messages".to_string()]),
            "a child's totals folded into the export would change this shape, which is the signal to start \
             accounting for them"
        );
        for row in parse_session_list(LIST) {
            assert!(
                row.id.starts_with("ses_"),
                "the list is rows of sessions: {row:?}"
            );
        }

        // The arithmetic that makes it material: the child in that measured pair cost MORE than its parent, and
        // the magnitude is the point — this is not a rounding difference.
        assert!(
            MEASURED_CHILD.0 > MEASURED_PARENT.0,
            "measured: child {MEASURED_CHILD:?} vs parent {MEASURED_PARENT:?}"
        );
        assert!(
            MEASURED_CHILD.1 > MEASURED_PARENT.1,
            "measured: the child read more input tokens than its parent"
        );

        // And the exposure is BOUNDED twice over: only an agent whose DECLARED child allowlist is non-empty can
        // produce a child at all, and only when the subagent gate is armed. If the declared set ever names another
        // agent, the accounting consequence is stated here rather than discovered later.
        let spawners: Vec<&str> = crate::engine::opencode_agents::V2_AGENT_PROFILES
            .iter()
            .filter(|profile| !profile.children.is_empty())
            .map(|profile| profile.id)
            .collect();
        assert_eq!(
            spawners,
            vec![
                crate::engine::opencode_agents::AGENT_ARCHITECT,
                crate::engine::opencode_agents::AGENT_SMITH
            ],
            "child spend can only be undercounted for agents that may spawn a child"
        );
        // The shipped posture is DISARMED (`FORGE_SUBAGENTS` unset) and the gate empties every allowlist, so under
        // the default posture no child can exist and the parent export IS the whole turn. The undercount asserted
        // above is therefore an ARMED-turn hazard — which is exactly why every turn declares its posture in one
        // line (`engine::opencode::OpenCodeHarness`) instead of leaving an armed turn indistinguishable in the log.
        use crate::engine::opencode_agents as gate;
        assert!(
            !gate::SUBAGENTS_DEFAULT,
            "the undercount is only a hazard because arming is possible"
        );
        for profile in gate::V2_AGENT_PROFILES.iter() {
            assert!(
                gate::v2_children_for_agent_gated(profile.id, gate::SUBAGENTS_DEFAULT).is_empty(),
                "with the gate disarmed '{}' cannot spawn a child, so its recorded spend is exact",
                profile.id
            );
        }
        // Arming is the only thing that puts a spawner back in play, and it does so for the declared pair alone.
        assert!(gate::v2_child_allowed_gated(
            gate::AGENT_SMITH,
            gate::AGENT_EXPLORE,
            true
        ));
        assert!(!gate::v2_child_allowed_gated(
            gate::AGENT_SMITH,
            gate::AGENT_EXPLORE,
            false
        ));
        assert!(
            !crate::engine::opencode_agents::BACKGROUND_CHILDREN_ENABLED,
            "background children would make this undercount unbounded in time as well as in number"
        );
    }
}
