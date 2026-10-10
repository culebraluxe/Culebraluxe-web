//! The form template: the XML contract, parsed — and the directory it is authored in.
//!
//! WHY THE FILES ARE READ AT RUNTIME, AND NEVER COMPILED IN. XML is the canonical authoring format for these documents,
//! and templates are versioned FILES: a live document re-renders from the exact version it was issued under, so a
//! version a record already points at must stay renderable. A new version, or a new template, is therefore an XML file
//! dropped in the directory — not a code change and not a rebuild. Nothing about a template is a constant here: the
//! directory is scanned, every file is parsed, and the library is keyed by `(id, version)`.
//!
//! WHY THE PARSER IS HAND-WRITTEN. The grammar is small, closed and specified below — one root element, four kinds of
//! child, attributes and inline `<value/>` references — and the workspace depends on no XML crate. A focused parser is
//! also the only way to keep the TypeScript contract's REFUSALS: a template that is wrong must be rejected with the
//! same reason on both sides rather than silently rendering a different document.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
mod parse_section;
mod show_all_value;
#[allow(unused_imports)]
pub use parse_section::*;
#[allow(unused_imports)]
pub use show_all_value::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// The real authoring directory, reached from this crate rather than from the working directory — a test that
    /// depended on the working directory would pass or fail by where it was run from.
    fn templates_dir_in_repo() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("forms/templates")
    }

    fn library() -> TemplateLibrary {
        TemplateLibrary::load_from_dir(&templates_dir_in_repo())
            .expect("the repository's templates load")
    }

    /// THE PRODUCTION ENTRY POINT, not the test helper. Its default is crate-relative, and the dev launcher starts the
    /// API from the repository root — so this assertion is what keeps a launcher detail from deciding whether documents
    /// can be composed at all.
    #[test]
    fn the_template_directory_resolves_from_any_working_directory() {
        let directory = templates_dir();
        assert!(
            directory.join("LISTING-01.v4.xml").exists(),
            "the templates did not resolve: {}",
            directory.display()
        );
    }

    /// THE LAUNCHER'S WORKING DIRECTORY — the one this actually broke in, and the one the assertion above never reached.
    /// `cargo test` starts a test binary in the CRATE directory, where the search finds `forms/templates` on the first
    /// candidate and stays green; `scripts/dev.sh` starts the API in the REPOSITORY ROOT, where `forms/templates` is
    /// nowhere, so the resolution fell through to that bare path and the forms screen reported "Cannot read the template
    /// directory forms/templates: No such file or directory (os error 2)".
    ///
    /// Both directories are named here, so neither can be the only one tested again.
    #[test]
    fn the_template_directory_resolves_from_the_repository_root_too() {
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        // CANONICALIZED, deliberately, and this is not cosmetic: `crate_dir.join("../..")` LOOKS like the repository root
        // but still carries the crate inside it lexically, so a search that walks up from it reaches the crate's own
        // `forms/templates` and reports a pass the launcher never gets. `scripts/dev.sh` `cd`s to the repository root and
        // starts the API there, so the root this must survive is the one with no `..` left in it.
        let repository_root = crate_dir
            .join("../..")
            .canonicalize()
            .expect("the repository root exists");
        assert!(
            repository_root.join("Cargo.toml").exists(),
            "this test's idea of the repository root is wrong: {}",
            repository_root.display()
        );
        for start in [crate_dir, repository_root.as_path()] {
            let directory = resolve_repo_path_from(start, DEFAULT_TEMPLATES_DIR);
            // ABSOLUTE FIRST, and this is the assertion the broken version fails: the resolver's last resort is the
            // bare `relative` path, which it returns for the ERROR MESSAGE to name — a relative path is then rescued or
            // not by whatever working directory asks, which is how `cargo test` (crate directory) stayed green while the
            // launcher (repository root) read nothing at all. A resolution that returns it has failed.
            assert!(
                directory.is_absolute(),
                "the templates did not resolve from {}: got the bare path {} — nothing was found",
                start.display(),
                directory.display()
            );
            assert!(
                directory.join("LISTING-01.v4.xml").exists(),
                "the templates did not resolve from {}: {}",
                start.display(),
                directory.display()
            );
        }
        // The resolver serves a SECOND base as well: the wordmark's default is REPOSITORY-relative
        // (`public/brand/CLLOGO.png`, `web/src/vault/artifact.rs`), so from the crate directory it is two ancestors up.
        // Asserted here because a search that knows one base fixes the templates by breaking the wordmark.
        let wordmark = resolve_repo_path_from(crate_dir, "public/brand/CLLOGO.png");
        assert!(
            wordmark.is_absolute() && wordmark.exists(),
            "a repository-relative default did not resolve: {}",
            wordmark.display()
        );
    }

    #[test]
    fn every_repository_template_parses() {
        let library = library();
        assert!(
            library.all().len() >= 9,
            "expected the nine authored versions, found {}",
            library.all().len()
        );
        let ids: Vec<&str> = library.families().into_iter().map(|(id, _)| id).collect();
        for expected in [
            "OFFER-01",
            "LISTING-01",
            "PR-PNS",
            "PR-PNS-AMD",
            "SHOW-INFO",
            "SHOW-RPT",
        ] {
            assert!(
                ids.contains(&expected),
                "template family {expected} is missing from the directory"
            );
        }
    }

    #[test]
    fn a_persisted_version_resolves_to_that_exact_version() {
        let library = library();
        // The reason versions are files rather than one mutable active template: live records point at exact versions.
        for version in [2, 3, 4, 5] {
            let template = library
                .version("LISTING-01", version)
                .unwrap_or_else(|| panic!("LISTING-01 v{version} is not loadable"));
            assert_eq!(template.version, version);
        }
        assert_eq!(library.newest("LISTING-01").map(|t| t.version), Some(5));
        assert!(library.version("LISTING-01", 99).is_none());
    }

    /// THE CANARY. The newest authored file is only a draft; which version a NEW document may use is a human decision that
    /// lives in `lib/forms/template-registry.ts` (`ACTIVE_TEMPLATE_VERSIONS`). The two agree today, and this test is what
    /// keeps that true: a failure here means an XML was dropped whose version nobody approved — either approve it in the
    /// registry in the same change, or remove the file.
    #[test]
    fn the_newest_authored_version_of_each_family_is_the_approved_one() {
        let library = library();
        for (id, approved) in [
            ("OFFER-01", 3),
            ("LISTING-01", 5),
            ("PR-PNS", 4),
            ("PR-PNS-AMD", 1),
            ("SHOW-INFO", 1),
            ("SHOW-RPT", 2),
        ] {
            assert_eq!(
                library.newest(id).map(|template| template.version),
                Some(approved),
                "{id}: the newest XML is not the approved version — see ACTIVE_TEMPLATE_VERSIONS"
            );
        }
    }

    #[test]
    fn a_template_carries_its_fields_sections_participants_and_signatures() {
        let library = library();
        let template = library
            .version("LISTING-01", 4)
            .expect("LISTING-01 v4 loads");

        assert_eq!(
            template.rendering.presentation,
            TemplatePresentation::Agreement
        );
        assert_eq!(template.document_type_label, "Listing Agreement");
        assert!(template.rendering.issuer.starts_with("Culebraluxe"));

        let price = template.field("listPrice").expect("listPrice exists");
        assert_eq!(price.field_type, TemplateFieldType::Money);
        assert!(price.required);
        assert_eq!(price.label, "Asking Price");

        let listing_type = template.field("listingType").expect("listingType exists");
        assert_eq!(listing_type.options.len(), 2);
        assert_eq!(listing_type.options[0], "Exclusive Right to Sell");

        let partnership = template
            .sections
            .iter()
            .find(|section| section.name == "partnership")
            .expect("the partnership section exists");
        assert!(!partnership.editable);
        assert!(
            partnership
                .segments
                .iter()
                .any(|segment| *segment == TemplateSectionSegment::Value("sellerName".into())),
            "the prose interpolates the seller's name"
        );
        assert!(partnership.values.contains(&"sellerName".to_string()));

        let seller = template
            .participants
            .iter()
            .find(|participant| participant.role == "SELLER")
            .expect("SELLER is a participant role");
        assert!(seller.multiple);

        assert_eq!(template.signature_groups.len(), 2);
        assert_eq!(template.signature_groups[0].role, "SELLER");
        assert!(template.signature_groups[0].initials);
        assert_eq!(
            template.signature_groups[0].field.as_deref(),
            Some("sellerName")
        );
    }

    #[test]
    fn an_empty_section_carries_no_segments() {
        let template = parse_template_xml(
            r#"<form id="T" version="1" title="T">
                 <field id="a" label="A" type="text"/>
                 <section id="empty" title="Empty" editable="true"></section>
               </form>"#,
        )
        .expect("an empty section is allowed");
        assert!(template.sections[0].segments.is_empty());
        assert!(template.sections[0].editable);
    }

    #[test]
    fn a_template_that_is_wrong_is_refused_with_a_reason() {
        let cases: [(&str, &str); 7] = [
            (
                "<form id=\"T\" version=\"1\" title=\"T\"><bogus/></form>",
                "Unknown element",
            ),
            (
                "<form id=\"T\" version=\"1\" title=\"T\"><field id=\"a\" label=\"A\" type=\"text\"/><field id=\"a\" label=\"A\" type=\"text\"/></form>",
                "Duplicate field id",
            ),
            (
                "<form id=\"T\" version=\"1\" title=\"T\"><field id=\"a\" label=\"A\" type=\"select\"/></form>",
                "non-empty options list",
            ),
            (
                "<form id=\"T\" version=\"1\" title=\"T\"><field id=\"a\" label=\"A\" type=\"text\" source=\"nowhere\"/></form>",
                "unknown source binding",
            ),
            (
                "<form id=\"T\" version=\"1\" title=\"T\" presentation=\"poem\"><field id=\"a\" label=\"A\" type=\"text\"/></form>",
                "presentation must be one of",
            ),
            (
                "<form id=\"T\" version=\"1\" title=\"T\"><field id=\"a\" label=\"A\" type=\"text\" when=\"novalue\"/></form>",
                "when must be",
            ),
            (
                "<form id=\"T\" version=\"1\" title=\"T\"><section id=\"s\" title=\"S\"><value field=\"missing\"/></section></form>",
                "references unknown field",
            ),
        ];
        for (xml, expected) in cases {
            let error = parse_template_xml(xml).expect_err("the template is refused");
            assert!(
                error.message.contains(expected),
                "expected a refusal mentioning \"{expected}\", got \"{}\"",
                error.message
            );
        }
        assert!(
            parse_template_xml(
                "<form id=\"T\" version=\"1\" title=\"T\"><field id=\"a\" label=\"A\" type=\"text\"/>"
            )
            .is_err(),
            "an unclosed document is refused"
        );
        let root = parse_template_xml("<letter id=\"T\" version=\"1\" title=\"T\"/>")
            .expect_err("a foreign root element is refused");
        assert!(root.message.contains("root must be <form>"));
    }

    #[test]
    fn a_when_gate_reads_the_named_field_case_insensitively() {
        let values = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
            pairs
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect()
        };
        let gate = TemplateWhen {
            field: "financing".to_string(),
            values: vec!["Cash".to_string(), "Blend".to_string()],
        };
        assert!(TemplateDefinition::when_satisfied(None, &values(&[])));
        assert!(TemplateDefinition::when_satisfied(
            Some(&gate),
            &values(&[("financing", "cash")])
        ));
        assert!(!TemplateDefinition::when_satisfied(
            Some(&gate),
            &values(&[("financing", "Bank")])
        ));
        assert!(
            !TemplateDefinition::when_satisfied(Some(&gate), &values(&[])),
            "an unset gate is hidden rather than shown"
        );
        assert!(TemplateDefinition::when_satisfied(
            Some(&gate),
            &values(&[("financing", "Show All")])
        ));
    }
}
