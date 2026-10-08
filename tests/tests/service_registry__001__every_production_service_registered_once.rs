//! SERVICE.REGISTRY — every production service registered once (TST-SERVICE-REGISTRY-001).
//!
//! CONTRACT. The production registry refuses a second registration for a domain
//! (`ServiceRegistry::new`, `web/src/service_kernel.rs:63` → `SERVICE_ALREADY_REGISTERED`), so the
//! static precondition must hold: every `AbstractService` descriptor in the tree names a domain
//! exactly once. Descriptors are declared two ways — the `abstract_service!` macro
//! (`web/src/composition.rs:75`, domain is the second argument) and manual `fn descriptor`
//! implementations (domain literal on the `ServiceDescriptor`, e.g. `web/src/service_gateway.rs`
//! and `forge/src/service.rs`, where it may be a const such as `FORGE_SERVICE_DOMAIN`).
//!
//! So: the census finds every descriptor domain across `web/src` and `forge/src`, every domain is
//! unique, the estate is the size production claims, and the known domains are present.
//!
//! NEGATIVE CASES. A census that cannot flag a duplicate would pass on a tree that registers
//! twice. So the uniqueness checker is also run against a planted duplicate (must name it) and a
//! clean list (must stay quiet). Test fakes under `#[cfg(test)]` are excluded from the census —
//! they are never registered.
//!
//! ISOLATION. L1 Component, harness AbstractServiceHarness — filesystem reads only, no database,
//! no network, no PROD. Deterministic.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test service_registry__001__every_production_service_registered_once

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use test_harness::source;

const HARNESS: &str = "AbstractServiceHarness/L1 Component";

/// First duplicate in `domains`, or `None` when every domain is registered once.
fn first_duplicate(domains: &[String]) -> Option<String> {
    let mut seen = HashSet::new();
    for domain in domains {
        if !seen.insert(domain.clone()) {
            return Some(domain.clone());
        }
    }
    None
}

/// String literals in `span`, in order — the macro's domain is the first one.
fn string_literals(span: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = span;
    while let Some(open) = rest.find('"') {
        rest = &rest[open + 1..];
        if let Some(close) = rest.find('"') {
            out.push(rest[..close].to_owned());
            rest = &rest[close + 1..];
        } else {
            break;
        }
    }
    out
}

/// Text from the opening `(` at `start` through its balanced `)`.
fn balanced_paren(text: &str, start: usize) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    for (offset, byte) in bytes.iter().enumerate().skip(start) {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..=offset]);
                }
            }
            _ => {}
        }
    }
    None
}

/// `const NAME … = "value"` pairs in `text`, so descriptor domains named by const resolve.
fn const_table(text: &str) -> HashMap<String, String> {
    let mut table = HashMap::new();
    for line in text.lines() {
        let code = source::code_of(line);
        let Some(eq) = code.find('=') else { continue };
        let Some(literal) = string_literals(&code[eq..]).into_iter().next() else { continue };
        let left = code[..eq].trim();
        if !left.starts_with("const ") && !left.starts_with("pub const ") {
            continue;
        }
        if let Some(name) = left.split_whitespace().skip_while(|token| *token != "const").nth(1) {
            let name = name.trim_end_matches(':');
            if !name.is_empty() && name != "const" {
                table.insert(name.to_owned(), literal);
            }
        }
    }
    table
}

/// Every descriptor domain declared in `path`: macro domains plus manual `fn descriptor` domains.
fn descriptor_domains(path: &PathBuf, text: &str, consts: &HashMap<String, String>) -> Vec<String> {
    let mut domains = Vec::new();
    for (index, _) in text.match_indices("abstract_service!") {
        let Some(open) = text[index..].find('(') else { continue };
        let Some(span) = balanced_paren(text, index + open) else { continue };
        match string_literals(span).into_iter().next() {
            Some(domain) => domains.push(domain),
            None => panic!(
                "{HARNESS}: {} has an abstract_service! without a domain literal",
                source::relative(path)
            ),
        }
    }
    let lines: Vec<&str> = text.lines().collect();
    for (number, line) in lines.iter().enumerate() {
        let code = source::code_of(line);
        if !code.contains("fn descriptor(") {
            continue;
        }
        if code.contains("ForgeServiceDescriptor") {
            // The Forge role registry is a different boundary (AbstractForgeService):
            // out of scope for the service registry census.
            continue;
        }
        if !code.contains("ServiceDescriptor") {
            // Only the AbstractService descriptor declares a registry domain.
            continue;
        }
        let end = (number + 8).min(lines.len());
        let window = &lines[number..end];
        let mut found = None;
        let mut template = false;
        for candidate in window {
            let code = source::code_of(candidate);
            let Some(at) = code.find("domain:") else { continue };
            let after = code[at + "domain:".len()..].trim();
            if after.contains('$') {
                // The abstract_service! template itself names `$domain`: a pattern, not a registration.
                template = true;
                break;
            }
            if let Some(literal) = string_literals(after).into_iter().next() {
                found = Some(literal);
            } else {
                let ident: String = after
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                found = Some(
                    consts.get(&ident).unwrap_or_else(|| panic!(
                        "{HARNESS}: {}:{} names domain by unresolvable const {ident}",
                        source::relative(path),
                        number + 1
                    )).clone(),
                );
            }
            break;
        }
        if template {
            continue;
        }
        match found {
            Some(domain) => domains.push(domain),
            None => panic!(
                "{HARNESS}: {}:{} declares a descriptor with no domain",
                source::relative(path),
                number + 1
            ),
        }
    }
    domains
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SERVICE-REGISTRY-001); the file and the assay use it.
fn service_registry_001__every_production_service_registered_once() {
    // ── NEGATIVE CONTROL FIRST: the checker must fire on a planted duplicate. ──
    assert_eq!(
        first_duplicate(&["person".into(), "contract".into(), "person".into()]),
        Some("person".into()),
        "{HARNESS}: the census must name a duplicated domain"
    );
    assert_eq!(
        first_duplicate(&["person".into(), "contract".into()]),
        None,
        "{HARNESS}: the census must stay quiet on a clean list"
    );

    // ── THE CENSUS: every descriptor domain in the production tree. ──
    let root = source::workspace_root();
    let mut consts = HashMap::new();
    let mut files = Vec::new();
    for dir in ["web/src", "forge/src"] {
        let dir_path = root.join(dir);
        let sources = source::sources_under(&dir_path);
        assert!(
            !sources.is_empty(),
            "{HARNESS}: {dir} must exist and hold sources, or the census is blind"
        );
        for path in sources {
            let text = source::read(&path);
            consts.extend(const_table(&text));
            files.push((path, text));
        }
    }
    let mut domains = Vec::new();
    for (path, text) in &files {
        // Test fakes are never registered: drop the test module, and only the test
        // module — a `#[cfg(test)]` on a `use` (e.g. service_gateway.rs:16) is not one.
        let lines: Vec<&str> = text.lines().collect();
        let mut cut = lines.len();
        for (number, line) in lines.iter().enumerate() {
            if line.trim() != "#[cfg(test)]" {
                continue;
            }
            let next = lines[number + 1..]
                .iter()
                .map(|candidate| candidate.trim())
                .find(|candidate| !candidate.is_empty())
                .unwrap_or("");
            if next == "mod tests {" || next.starts_with("mod ") || next.starts_with("pub mod ") {
                cut = number;
                break;
            }
        }
        let production: String = lines[..cut].join("\n");
        domains.extend(descriptor_domains(path, &production, &consts));
    }

    assert!(
        domains.len() >= 30,
        "{HARNESS}: the estate must be census-sized (found {} domains); a silent walker would flatter an empty tree",
        domains.len()
    );
    for domain in &domains {
        assert!(
            !domain.is_empty()
                && domain
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "{HARNESS}: every domain is a lowercase slug, got {domain:?}"
        );
    }
    assert_eq!(
        first_duplicate(&domains),
        None,
        "{HARNESS}: every production service must be registered once — duplicate domains would be refused at startup"
    );
    for known in ["person", "contract", "property", "forge", "accounting", "whatsapp", "vault"] {
        assert!(
            domains.iter().any(|domain| domain == known),
            "{HARNESS}: the census must include the {known} service (found {} domains)",
            domains.len()
        );
    }
}
