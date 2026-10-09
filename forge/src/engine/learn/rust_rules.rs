//! Small, syntax-aware Rust observations used by the learning loop.
//!
//! These are review candidates, never confirmed defects. This module does no Git, database, or filesystem I/O.

use super::RuleObservation;
use proc_macro2::Span;
use syn::{
    spanned::Spanned,
    visit::{self, Visit},
    Expr, ExprCall, ExprMethodCall, ExprPath, ItemFn, ItemMod, Local, Pat,
};

const FALLIBLE_FUNCTIONS: &[&str] = &[
    "create_dir_all",
    "read",
    "read_dir",
    "read_to_string",
    "remove_dir_all",
    "remove_file",
    "rename",
    "write",
    "write_all",
    "execute",
    "fetch_one",
    "fetch_optional",
    "commit",
    "rollback",
    "flush",
    "sync_all",
    "spawn",
    "output",
    "status",
];

// Best-effort removal is an intentional cleanup idiom in this repository. Keep this list narrow;
// authoritative writes and state transitions must remain visible to the rule.
const INTENTIONAL_CLEANUP_FUNCTIONS: &[&str] = &["remove_file", "remove_dir_all", "rollback"];

#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct ParseIssue {
    pub(super) path: String,
    pub(super) message: String,
}

pub(super) fn scan_rust_files(
    files: &[(String, String)],
    revision: &str,
) -> (Vec<RuleObservation>, Vec<ParseIssue>) {
    let mut observations = Vec::new();
    let mut issues = Vec::new();

    for (path, source) in files.iter().filter(|(path, _)| scan_path(path)) {
        match syn::parse_file(source) {
            Ok(file) => {
                let mut visitor = RuleVisitor {
                    path,
                    source,
                    revision,
                    current_function: None,
                    observations: Vec::new(),
                };
                visitor.visit_file(&file);
                observations.extend(visitor.observations);
            }
            Err(error) => issues.push(ParseIssue {
                path: path.clone(),
                message: error.to_string(),
            }),
        }
    }

    observations.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.start_line.cmp(&b.start_line))
            .then(a.rule_id.cmp(&b.rule_id))
    });
    (observations, issues)
}

fn scan_path(path: &str) -> bool {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    path.ends_with(".rs")
        && !file_name.starts_with("test_")
        && !file_name.ends_with("_test.rs")
        && !file_name.ends_with("_tests.rs")
        && !path.starts_with("tests/")
        && !path.contains("/tests/")
        && !path.starts_with("target/")
        && !path.contains("/target/")
        && !path.starts_with("build/")
        && !path.contains("/build/")
        && !path.starts_with("dist/")
        && !path.contains("/dist/")
        && !path.starts_with("out/")
        && !path.contains("/out/")
        && !path.starts_with("vendor/")
        && !path.contains("/vendor/")
        && !path.starts_with("generated/")
        && !path.contains("/generated/")
}

struct RuleVisitor<'a> {
    path: &'a str,
    source: &'a str,
    revision: &'a str,
    current_function: Option<String>,
    observations: Vec<RuleObservation>,
}

impl RuleVisitor<'_> {
    fn add(
        &mut self,
        rule_id: &str,
        span: Span,
        rationale: &str,
        confidence: &str,
        limitation: &str,
    ) {
        let start = span.start().line.max(1);
        let end = span.end().line.max(start);
        let context = self
            .source
            .lines()
            .skip(start.saturating_sub(1))
            .take(end.saturating_sub(start) + 1)
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let normalized_context = context.chars().take(200).collect::<String>();
        let path = self.path.to_string();
        let key = format!("{rule_id}:{path}:{start}");
        self.observations.push(RuleObservation {
            rule_id: rule_id.to_string(),
            rule_version: 1,
            key,
            source_revision: self.revision.to_string(),
            path,
            start_line: start,
            end_line: end,
            normalized_context,
            rationale: rationale.to_string(),
            severity: "review".to_string(),
            confidence: confidence.to_string(),
            limitations: limitation.to_string(),
        });
    }
}

impl<'ast> Visit<'ast> for RuleVisitor<'_> {
    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        let previous = self.current_function.replace(item.sig.ident.to_string());
        visit::visit_item_fn(self, item);
        self.current_function = previous;
    }

    fn visit_item_mod(&mut self, item: &'ast ItemMod) {
        if item.attrs.iter().any(|attribute| {
            attribute.path().is_ident("cfg")
                && attribute
                    .parse_args::<syn::Ident>()
                    .is_ok_and(|ident| ident == "test")
        }) {
            return;
        }
        visit::visit_item_mod(self, item);
    }

    fn visit_local(&mut self, local: &'ast Local) {
        if matches!(local.pat, Pat::Wild(_)) {
            if let Some(initializer) = &local.init {
                if let Some(name) = known_fallible_call(&initializer.expr)
                    .filter(|name| !INTENTIONAL_CLEANUP_FUNCTIONS.contains(name))
                    .filter(|name| {
                        !intentional_session_marker_write(
                            self.path,
                            self.current_function.as_deref(),
                            name,
                        )
                    })
                {
                    self.add(
                        "RUST-DISCARDED-FALLIBLE-RESULT",
                        local.span(),
                        &format!("`let _` discards the result of `{name}` without a visible handler"),
                        "medium",
                        "The rule uses a small known-fallible API allowlist and does not type-check; intentional best-effort cleanup still needs human review.",
                    );
                }
            }
        }
        visit::visit_local(self, local);
    }

    fn visit_expr_match(&mut self, expression: &'ast syn::ExprMatch) {
        for arm in &expression.arms {
            if is_err_pattern(&arm.pat) && is_empty_block(&arm.body) {
                self.add(
                    "RUST-EMPTY-ERROR-ARM",
                    arm.span(),
                    "A `Result::Err` match arm has an empty body, so this branch visibly performs no handling.",
                    "low",
                    "Empty error arms can be deliberate for best-effort work. The scanner cannot infer the surrounding recovery contract.",
                );
            }
        }
        visit::visit_expr_match(self, expression);
    }

    fn visit_expr_method_call(&mut self, expression: &'ast ExprMethodCall) {
        let method = expression.method.to_string();
        if matches!(method.as_str(), "ok" | "unwrap_or_default")
            && authoritative_path(self.path)
            && likely_fallible_receiver(&expression.receiver)
            && !intentional_optional_metadata_read(self.path, self.current_function.as_deref())
        {
            self.add(
                "RUST-ERROR-TO-SUCCESS-FALLBACK",
                expression.span(),
                &format!("`.{method}()` may erase an error from an authoritative service or database path"),
                "low",
                "Syntax alone cannot distinguish Result from Option or establish whether this fallback is contractually safe; this is a review-only candidate, not a defect finding.",
            );
        }
        visit::visit_expr_method_call(self, expression);
    }
}

fn intentional_optional_metadata_read(path: &str, function: Option<&str>) -> bool {
    (path == "forge/src/engine/learn.rs" && function == Some("read_anchor_secs"))
        || (matches!(
            path,
            "forge/src/engine/maestro.rs" | "forge/src/engine/opencode/config.rs"
        ) && function == Some("read_session_id"))
}

fn intentional_session_marker_write(
    path: &str,
    containing_function: Option<&str>,
    function: &str,
) -> bool {
    function == "write"
        && containing_function == Some("write_session_id")
        && matches!(
            path,
            "forge/src/engine/maestro.rs" | "forge/src/engine/opencode/config.rs"
        )
}

fn authoritative_path(path: &str) -> bool {
    path.starts_with("db/src/")
        || path.starts_with("forge/src/engine/")
        || path.contains("/services/")
}

fn is_err_pattern(pattern: &Pat) -> bool {
    match pattern {
        Pat::TupleStruct(tuple) => tuple
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Err"),
        _ => false,
    }
}

fn is_empty_block(expression: &Expr) -> bool {
    matches!(expression, Expr::Block(block) if block.block.stmts.is_empty())
}

fn known_fallible_call(expression: &Expr) -> Option<&'static str> {
    match expression {
        Expr::Call(ExprCall { func, .. }) => match func.as_ref() {
            Expr::Path(ExprPath { path, .. }) => path
                .segments
                .last()
                .map(|segment| segment.ident.to_string())
                .and_then(|name| {
                    FALLIBLE_FUNCTIONS
                        .iter()
                        .copied()
                        .find(|item| *item == name)
                }),
            _ => None,
        },
        Expr::MethodCall(call) => FALLIBLE_FUNCTIONS
            .iter()
            .copied()
            .find(|item| *item == call.method.to_string()),
        Expr::Await(awaited) => known_fallible_call(&awaited.base),
        Expr::Paren(paren) => known_fallible_call(&paren.expr),
        Expr::Group(group) => known_fallible_call(&group.expr),
        Expr::Try(_) => None,
        _ => None,
    }
}

fn likely_fallible_receiver(expression: &Expr) -> bool {
    match expression {
        Expr::Call(call) => known_fallible_call(&Expr::Call(call.clone())).is_some(),
        Expr::MethodCall(call) => FALLIBLE_FUNCTIONS
            .iter()
            .any(|name| *name == call.method.to_string()),
        Expr::Await(awaited) => likely_fallible_receiver(&awaited.base),
        Expr::Paren(paren) => likely_fallible_receiver(&paren.expr),
        Expr::Group(group) => likely_fallible_receiver(&group.expr),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syntax_observations_ignore_comments_strings_tests_and_propagated_results() {
        let source = r#"
            // let _ = std::fs::write("comment", "x");
            const EXAMPLE: &str = "catch {} .unwrap_or_default()";
            fn safe() -> Result<(), std::io::Error> {
                std::fs::write("file", "data")?;
                Ok(())
            }
            #[cfg(test)]
            mod tests {
                fn ignored_fixture() { let _ = std::fs::write("x", "y"); }
            }
        "#;
        let (observations, issues) =
            scan_rust_files(&[("db/src/example.rs".into(), source.into())], "abc123");
        assert!(issues.is_empty());
        assert!(observations.is_empty());
    }

    #[test]
    fn known_fallible_discard_and_empty_error_arm_are_review_only_with_stable_locations() {
        let source = "fn write() { let _ = std::fs::write(\"x\", \"y\"); }\nfn swallow() { match load() { Ok(_) => {}, Err(_) => {} } }";
        let files = [("db/src/example.rs".into(), source.into())];
        let (first, issues) = scan_rust_files(&files, "abc123");
        let (second, _) = scan_rust_files(&files, "abc123");
        assert!(issues.is_empty());
        assert_eq!(first, second);
        assert!(first
            .iter()
            .any(|finding| finding.rule_id == "RUST-DISCARDED-FALLIBLE-RESULT"));
        assert!(first
            .iter()
            .any(|finding| finding.rule_id == "RUST-EMPTY-ERROR-ARM"));
        assert!(first.iter().all(|finding| finding.severity == "review"));
        assert!(first
            .iter()
            .all(|finding| finding.confidence != "confirmed"));
    }

    #[test]
    fn best_effort_cleanup_and_rust_test_fixtures_are_narrow_exceptions() {
        let cleanup = "fn cleanup() { let _ = std::fs::remove_file(\"temp\"); }";
        let (findings, issues) =
            scan_rust_files(&[("forge/src/cleanup.rs".into(), cleanup.into())], "rev");
        assert!(issues.is_empty());
        assert!(findings.is_empty());

        let metadata = r#"
            fn read_session_id(workspace: &str) -> Option<String> {
                let raw = fs::read_to_string(session_marker_path(workspace)).ok()?;
                Some(raw)
            }
            fn write_session_id(workspace: &str, id: &str) {
                let path = session_marker_path(workspace);
                let _ = fs::write(path, format!("{id}\\n"));
            }
            fn rollback(tx: Tx) {
                let _ = tx.rollback().await;
            }
        "#;
        let (findings, issues) = scan_rust_files(
            &[("forge/src/engine/maestro.rs".into(), metadata.into())],
            "rev",
        );
        assert!(issues.is_empty());
        assert!(findings.is_empty());

        let fixture = "fn test_fixture() { let _ = std::fs::write(\"x\", \"y\"); }";
        let (findings, issues) = scan_rust_files(
            &[("forge/src/sample_test.rs".into(), fixture.into())],
            "rev",
        );
        assert!(issues.is_empty());
        assert!(findings.is_empty());
    }

    #[test]
    fn default_fallback_is_review_candidate_only_on_authoritative_paths() {
        let source = "fn read() { let value = storage.read().unwrap_or_default(); }";
        let (authoritative, _) =
            scan_rust_files(&[("db/src/read.rs".into(), source.into())], "rev");
        let (ordinary, _) = scan_rust_files(&[("forge/src/ui.rs".into(), source.into())], "rev");
        assert!(authoritative
            .iter()
            .any(|finding| finding.rule_id == "RUST-ERROR-TO-SUCCESS-FALLBACK"));
        assert!(!ordinary
            .iter()
            .any(|finding| finding.rule_id == "RUST-ERROR-TO-SUCCESS-FALLBACK"));
    }

    #[test]
    fn unsupported_rust_syntax_is_reported_as_a_parse_issue() {
        let (observations, issues) =
            scan_rust_files(&[("forge/src/lib.rs".into(), "fn {".into())], "rev");
        assert!(observations.is_empty());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].path, "forge/src/lib.rs");
    }
}
