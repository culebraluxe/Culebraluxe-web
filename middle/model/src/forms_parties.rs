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
            (
                // The Owner's jurisdiction is not the Property's, and this Agreement attests only that the company is
                // the Owner and who binds it — never where the company is registered. The sentence used to read "a
                // {state} entity registered with the Puerto Rico Department of State under Register No. {register}",
                // which printed "a LLC entity registered with the Puerto Rico Department of State under Register No. ,"
                // in the first paragraph of Jessica's listing when neither answer was a jurisdiction and a number
                // (2026-10-12). The answers are still on the form; they are simply not attested to here.
                format!("{company}, represented by {representative}, {title}"),
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
        // "Florida" and "12345" are saved answers, and the Agreement still attests to neither — no jurisdiction and no
        // register number in the Owner's description, only who the company is and who binds it. This test used to pass
        // with a sentence claiming Puerto Rico Department of State registration, which is the whole defect: nobody knows
        // where the company is registered, and a listing agreement does not need to (2026-10-12).
        assert_eq!(
            d.description,
            "Caribe Holdings LLC, represented by Jessica Iverson, Managing Member"
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
}
