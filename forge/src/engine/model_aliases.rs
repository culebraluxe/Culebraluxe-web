//! Shorthand → real OpenCode model names.
//!
//! The lane configs, prompts and command lines speak in shorthands (`flash`, `v4-flash`,
//! `muse-contributor`, `longcat`…); OpenCode bills the full `vendor/model` id. This module is the one
//! place that translates, so a shorthand typed anywhere resolves to the same real name every time —
//! never silently to `deepseek/deepseek-flash` because the alias was missed.
//!
//! Resolution is fail-closed on ambiguity of policy: an unknown shorthand is an error naming the
//! accepted aliases, not a vendor default.

/// One alias row: every accepted shorthand maps to one real OpenCode model id.
const ALIASES: &[(&[&str], &str)] = &[
    (
        &["flash", "deepseek-flash", "deepseek_flash"],
        "deepseek/deepseek-flash",
    ),
    (
        &[
            "v4-flash",
            "deepseek-v4-flash",
            "v4_flash",
            "deepseek_v4_flash",
        ],
        "deepseek/deepseek-v4-flash",
    ),
    (
        &[
            "v4-flash-vision",
            "v4-flash-vision-exp",
            "deepseek-v4-flash-vision-exp",
        ],
        "deepseek/deepseek-v4-flash-vision-exp",
    ),
    (
        &["v4-pro", "deepseek-v4-pro", "deepseek_v4_pro"],
        "deepseek/deepseek-v4-pro",
    ),
    (&["muse-1.1", "muse_1_1"], "meta/muse-spark-1.1"),
    (&["muse-1.2", "muse_1_2"], "meta/muse-spark-1.2"),
    (
        &["muse-1.2-contributor", "muse_1_2_contributor"],
        "meta/muse-spark-1.2-contributor",
    ),
    (
        &["muse", "muse-1.3", "muse_1_3", "muse-spark-1.3"],
        "meta/muse-spark-1.3",
    ),
    (
        &[
            "muse-contributor",
            "muse-1.3-contributor",
            "muse_contributor",
            "muse-contributor-1.3",
        ],
        "meta/muse-spark-1.3-contributor",
    ),
    (
        &["big-pickle", "big_pickle", "pickle"],
        "opencode/big-pickle",
    ),
    (
        &["fledge", "fledge-alpha", "fledgealpha"],
        "opencode/fledge-alpha-free",
    ),
    (
        &["ling", "ling-3.1", "ling-3.1-flash"],
        "opencode/ling-3.1-flash-free",
    ),
    (&["ling-3.0"], "opencode/ling-3.0-flash-fin-free"),
    (
        &["longcat", "longcat-2.5"],
        "opencode/longcat-2.5-preview-free",
    ),
    (&["mimo", "mimo-v2.6"], "opencode/mimo-v2.6-flash-free"),
    (
        &["muse-free", "muse-contributor-free"],
        "opencode/muse-spark-1.3-contributor-free",
    ),
    (
        &["nemotron", "nemotron-3-ultra"],
        "opencode/nemotron-3-ultra-free",
    ),
    (
        &["nemotron-lightning", "nemotron-3.5-lightning", "lightning"],
        "opencode/nemotron-3.5-lightning-free",
    ),
    (
        &["spacebunny", "space-bunny", "space_bunny"],
        "opencode/space-bunny-free",
    ),
];

/// Resolve a shorthand or a full `vendor/model` id to the real OpenCode model id.
///
/// - A full id (`vendor/model`, contains `/`) passes through trimmed — the explicit form is already
///   the real name and naming a model OpenCode does not know is OpenCode's own fail at dispatch.
/// - A known shorthand maps to its real id.
/// - Anything else is a configuration error naming the accepted aliases. Never silently a default.
pub fn normalize_model_name(raw: &str) -> workflow::Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(workflow::WorkflowError::generic(
            "empty model name: set OPENCODE_MODEL to a shorthand (flash, v4-flash, muse-contributor, \
             longcat, mimo, nemotron, ...) or a full vendor/model id",
        ));
    }
    if trimmed.contains('/') {
        return Ok(trimmed.to_string());
    }
    let key = trimmed.to_ascii_lowercase();
    for (aliases, real) in ALIASES {
        if aliases.iter().any(|alias| *alias == key) {
            return Ok(real.to_string());
        }
    }
    Err(workflow::WorkflowError::generic(format!(
        "unknown model shorthand {trimmed:?}: use a full vendor/model id or one of: {}",
        ALIASES
            .iter()
            .flat_map(|(aliases, _)| aliases.iter().copied())
            .collect::<Vec<_>>()
            .join(", ")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_ids_pass_through() {
        assert_eq!(
            normalize_model_name("deepseek/deepseek-v4-flash").expect("passthrough"),
            "deepseek/deepseek-v4-flash"
        );
        assert_eq!(
            normalize_model_name("  meta/muse-spark-1.3-contributor ").expect("passthrough"),
            "meta/muse-spark-1.3-contributor"
        );
    }

    #[test]
    fn shorthands_resolve_to_real_names() {
        assert_eq!(
            normalize_model_name("v4-flash").expect("alias"),
            "deepseek/deepseek-v4-flash"
        );
        assert_eq!(
            normalize_model_name("V4-FLASH").expect("case-insensitive"),
            "deepseek/deepseek-v4-flash"
        );
        assert_eq!(
            normalize_model_name("muse-contributor").expect("alias"),
            "meta/muse-spark-1.3-contributor"
        );
        assert_eq!(
            normalize_model_name("flash").expect("alias"),
            "deepseek/deepseek-flash"
        );
        assert_eq!(
            normalize_model_name("longcat").expect("alias"),
            "opencode/longcat-2.5-preview-free"
        );
        assert_eq!(
            normalize_model_name("spacebunny").expect("alias"),
            "opencode/space-bunny-free"
        );
    }

    #[test]
    fn unknown_shorthand_fails_closed() {
        let error = normalize_model_name("gpt-9-turbo").expect_err("must fail");
        assert!(
            error.to_string().contains("unknown model shorthand"),
            "{error}"
        );
        assert!(normalize_model_name("   ").is_err(), "blank is an error");
    }
}
