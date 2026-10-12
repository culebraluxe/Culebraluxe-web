//! Who a party is, worked out from the answers on the form, so the parties clause and the signature lines say what is true
//! for each kind of party, and the right person is the one asked to sign.
//!
//! THE THREE KINDS. An individual signs for themselves. An entity (a company) is bound by the person who represents it,
//! so the line reads "Company, by Representative, Title" and the representative is the one who signs. A party acting
//! through an attorney-in-fact is bound by that agent under a power, so the line reads "Party, by Agent, Attorney-in-Fact"
//! and the agent is the one who signs.
//!
//! THE SPOUSE. A married individual (or a principal) has a spouse who signs, unless recorded capitulaciones (a prenuptial
//! agreement) say otherwise. The spouse is named in the parties clause either way; only the signing depends on the answer.
//!
//! These values are DERIVED: they are written on save and never typed. A form that has not answered the question gets an
//! empty line, and the send refuses a block that has a name but no email.
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedParty {
    pub description: String,
    pub sign_block: String,
    pub signer_name: String,
    pub spouse_sign: String,
    pub spouse_sign_email: String,
}

/// The templates whose parties are derived, and the prefixes of their parties.
pub fn party_prefixes(template_id: &str) -> &'static [&'static str] {
    match template_id {
        "PR-PNS" => &["buyer", "seller"],
        "LISTING-01" => &["seller"],
        "OFFER-01" => &["buyer"],
        _ => &[],
    }
}

/// Writes the derived values for every party of this template into `values`.
pub fn apply_party_derivations(template_id: &str, values: &mut BTreeMap<String, String>) {
    // A listing that already names a cobroker has one: the cobroker block and acknowledgment follow the name, so a form
    // converted from a version that had no cobroker switch carries its cobroker onto the page.
    if template_id == "LISTING-01"
        && values
            .get("hasCobroker")
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
        && ["cobrokerName", "cobrokerEmail"].iter().any(|key| {
            values
                .get(*key)
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
        })
    {
        values.insert("hasCobroker".to_owned(), "Yes".to_owned());
    }
    for prefix in party_prefixes(template_id) {
        let kind_key = format!("{prefix}Kind");
        if values
            .get(&kind_key)
            .map(|value| value.trim())
            .unwrap_or_default()
            .is_empty()
        {
            values.insert(kind_key.clone(), "Individual".to_owned());
        }
        let derived = derive_party(prefix, values);
        // The spouse's block is on the form only when the spouse signs. The gate reads this, not the civil status, so a
        // seller who was married and is now an entity (or has a prenup) has no spouse block.
        let spouse_on_form = if derived.spouse_sign.is_empty() {
            ""
        } else {
            "Yes"
        };
        values.insert(format!("{prefix}SpouseOnForm"), spouse_on_form.to_owned());
        values.insert(format!("{prefix}Description"), derived.description);
        values.insert(format!("{prefix}SignBlock"), derived.sign_block);
        values.insert(format!("{prefix}SignerName"), derived.signer_name);
        values.insert(format!("{prefix}SpouseSign"), derived.spouse_sign);
        values.insert(
            format!("{prefix}SpouseSignEmail"),
            derived.spouse_sign_email,
        );
    }
}

pub fn derive_party(prefix: &str, values: &BTreeMap<String, String>) -> DerivedParty {
    let get = |suffix: &str| {
        values
            .get(&format!("{prefix}{suffix}"))
            .map(|value| value.trim().to_owned())
            .unwrap_or_default()
    };
    let name = get("Name");
    let kind = get("Kind");
    let married = get("CivilStatus") == "Married";
    let prenup = get("Prenup") == "Yes";
    let spouse = get("SpouseName");
    let spouse_email = get("SpouseEmail");

    let (description, sign_block, signer_name, signs_spouse) = match kind.as_str() {
        "Entity" => {
            let company = get("EntityName");
            let representative = get("RepName");
            let title = get("RepTitle");
            // The registration clause is a fact about the company, and it prints only when the answer behind it is there
            // to print. A blank "PR Department of State Register No." used to produce "…, a LLC entity registered with
            // the Puerto Rico Department of State under Register No. , represented by …" in the first paragraph of
            // Jessica's listing — a claim about a company nobody had checked, and one the Property being in Puerto Rico
            // does not make true (2026-10-12). Fill the number in and the clause comes back; leave it blank and the Owner
            // is described by name and by who binds it, which is what a listing agreement attests to. The jurisdiction
            // answer ("State of Formation") is deliberately not printed: it may hold a type ("LLC"), and where a company
            // was formed is not a fact this Agreement needs.
            let register = get("EntityRegister");
            let registration = if register.trim().is_empty() {
                String::new()
            } else {
                format!(
                    ", registered with the Puerto Rico Department of State under Register No. {}",
                    register.trim()
                )
            };
            (
                format!("{company}{registration}, represented by {representative}, {title}"),
                format!("{company}, by {representative}, {title}"),
                representative,
                false,
            )
        }
        "Power of Attorney" => {
            let agent = get("PoaAgent");
            (
                format!(
                    "{name}, represented by {agent}, attorney-in-fact under a special power of attorney dated {date} before {notary}, Deed/Affidavit No. {deed}",
                    date = get("PoaDate"),
                    notary = get("PoaNotary"),
                    deed = get("PoaDeed"),
                ),
                format!("{name}, by {agent}, Attorney-in-Fact"),
                agent,
                true,
            )
        }
        _ => {
            let civil = if married {
                let mut text = format!("married to {spouse}");
                if prenup {
                    text.push_str(", under recorded capitulaciones");
                }
                text
            } else {
                get("CivilStatus").to_lowercase()
            };
            let description = if civil.is_empty() {
                format!("{name}, of legal age")
            } else {
                format!("{name}, of legal age, {civil}")
            };
            (description, name.clone(), name, true)
        }
    };

    let spouse_signs = signs_spouse && married && !prenup && !spouse.is_empty();
    DerivedParty {
        description,
        sign_block,
        signer_name,
        spouse_sign: if spouse_signs { spouse } else { String::new() },
        spouse_sign_email: if spouse_signs {
            spouse_email
        } else {
            String::new()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn an_individual_married_without_a_prenup_names_and_signs_the_spouse() {
        let v = values(&[
            ("buyerName", "Ana Ruiz"),
            ("buyerKind", "Individual"),
            ("buyerCivilStatus", "Married"),
            ("buyerSpouseName", "Luis Ruiz"),
            ("buyerSpouseEmail", "luis@example.test"),
        ]);
        let d = derive_party("buyer", &v);
        assert_eq!(
            d.description,
            "Ana Ruiz, of legal age, married to Luis Ruiz"
        );
        assert_eq!(d.sign_block, "Ana Ruiz");
        assert_eq!(d.signer_name, "Ana Ruiz");
        assert_eq!(d.spouse_sign, "Luis Ruiz");
        assert_eq!(d.spouse_sign_email, "luis@example.test");
    }

    #[test]
    fn a_prenup_names_the_spouse_but_does_not_sign_for_them() {
        let v = values(&[
            ("sellerName", "Pedro Soto"),
            ("sellerKind", "Individual"),
            ("sellerCivilStatus", "Married"),
            ("sellerPrenup", "Yes"),
            ("sellerSpouseName", "Marta Soto"),
            ("sellerSpouseEmail", "marta@example.test"),
        ]);
        let d = derive_party("seller", &v);
        assert!(d
            .description
            .contains("married to Marta Soto, under recorded capitulaciones"));
        assert_eq!(d.spouse_sign, "");
        assert_eq!(d.spouse_sign_email, "");
    }

    #[test]
    fn a_company_is_bound_by_its_representative_who_signs() {
        let v = values(&[
            ("sellerName", "Caribe Holdings LLC"),
            ("sellerKind", "Entity"),
            ("sellerEntityName", "Caribe Holdings LLC"),
            ("sellerEntityState", "Florida"),
            ("sellerEntityRegister", "12345"),
            ("sellerRepName", "Jessica Iverson"),
            ("sellerRepTitle", "Managing Member"),
            ("sellerCivilStatus", "Married"),
        ]);
        let d = derive_party("seller", &v);
        // The register number is the answer that carries the clause: with it saved, the Owner is described as a company
        // registered with the Puerto Rico Department of State, and with it blank that sentence is not in the agreement.
        assert_eq!(
            d.description,
            "Caribe Holdings LLC, registered with the Puerto Rico Department of State under Register No. 12345, represented by Jessica Iverson, Managing Member"
        );
        assert_eq!(
            d.sign_block,
            "Caribe Holdings LLC, by Jessica Iverson, Managing Member"
        );
        assert_eq!(d.signer_name, "Jessica Iverson");
        assert_eq!(d.spouse_sign, "", "a company has no spouse who signs");
    }

    #[test]
    fn an_attorney_in_fact_signs_for_the_principal_under_the_power() {
        let v = values(&[
            ("sellerName", "Caribe Holdings LLC"),
            ("sellerKind", "Power of Attorney"),
            ("sellerPoaAgent", "Lisa Penfield"),
            ("sellerPoaDate", "2026-09-01"),
            ("sellerPoaNotary", "Maria Notary"),
            ("sellerPoaDeed", "42"),
        ]);
        let d = derive_party("seller", &v);
        assert_eq!(
            d.sign_block,
            "Caribe Holdings LLC, by Lisa Penfield, Attorney-in-Fact"
        );
        assert_eq!(d.signer_name, "Lisa Penfield");
        assert!(d.description.contains(
            "special power of attorney dated 2026-09-01 before Maria Notary, Deed/Affidavit No. 42"
        ));
    }

    #[test]
    fn an_unanswered_party_has_an_empty_line_not_a_guess() {
        let mut v = values(&[("buyerName", "Ana Ruiz")]);
        apply_party_derivations("PR-PNS", &mut v);
        assert_eq!(v.get("buyerKind").map(String::as_str), Some("Individual"));
        assert_eq!(v.get("buyerSpouseSign").map(String::as_str), Some(""));
    }

    #[test]
    fn a_company_with_a_blank_register_number_is_never_attested_to_be_registered() {
        // Jessica's own answers: an LLC whose State of Formation holds a type rather than a jurisdiction, and a blank
        // "PR Department of State Register No.". The Owner is named and who binds it is named, and the agreement claims
        // nothing about where the company is registered — this used to print "Bodysoulandbeauty LLC, a LLC entity
        // registered with the Puerto Rico Department of State under Register No. , represented by Jessica Iverson,
        // President" in the first paragraph of her listing (2026-10-12).
        let v = values(&[
            ("sellerKind", "Entity"),
            ("sellerName", "Bodysoulandbeauty LLC"),
            ("sellerEntityName", "Bodysoulandbeauty LLC"),
            ("sellerEntityState", "LLC"),
            ("sellerRepName", "Jessica Iverson"),
            ("sellerRepTitle", "President"),
        ]);
        let d = derive_party("seller", &v);
        assert_eq!(
            d.description,
            "Bodysoulandbeauty LLC, represented by Jessica Iverson, President"
        );
        assert!(
            !d.description.contains("Puerto Rico Department of State"),
            "a blank register number must not attest to a registration, got: {}",
            d.description
        );
        // A register number typed in brings the clause back — the company registered in Puerto Rico is still described.
        let mut registered = v.clone();
        registered.insert("sellerEntityRegister".into(), " 500123 ".into());
        let d = derive_party("seller", &registered);
        assert_eq!(
            d.description,
            "Bodysoulandbeauty LLC, registered with the Puerto Rico Department of State under Register No. 500123, represented by Jessica Iverson, President"
        );
    }
}
