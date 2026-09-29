//! Moved from `forms.rs` (move only): list_signer_people.

#[allow(unused_imports)]
use super::*;

impl FormDao {
    pub async fn list_signer_people(
        &self,
        form_instance_id: &str,
    ) -> DbResult<Vec<FormSignerPerson>> {
        let form = sqlx::query_as::<_, SignerFormRow>(
            r#"
            select f.deal_id::text as deal_id,
                   f.template_id,
                   f.status,
                   person.display_name as person_name,
                   person.id::text as resolved_person_id
            from document_form_instance f
            left join person on person.id = f.person_id
            where f.id = $1::uuid
            limit 1
            "#,
        )
        .bind(form_instance_id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("form.signers.form", &error))?;

        let Some(form) = form else {
            return Ok(vec![]);
        };

        let listing_direct_draft = form.template_id == LISTING_TEMPLATE_ID
            && form.status != "issued"
            && form.resolved_person_id.is_some();
        let mut people = Vec::new();

        if let Some(person_id) = form.resolved_person_id.clone() {
            let email = sqlx::query_scalar::<_, String>(
                r#"
                select identity_value
                from person_identity
                where person_id = $1::uuid and identity_type = 'email'
                order by is_primary desc, created_at asc
                limit 1
                "#,
            )
            .bind(&person_id)
            .fetch_optional(&mut *self.db.connection().await?)
            .await
            .map_err(|error| DbFailure::from_sqlx("form.signers.direct_email", &error))?;

            people.push(FormSignerPerson {
                            slot_id: None,
                person_id: Some(person_id),
                name: form.person_name.unwrap_or_default(),
                email,
                role: if listing_direct_draft {
                    "SELLER".into()
                } else {
                    "CLIENT".into()
                },
            });
        }

        if let Some(deal_id) = form.deal_id.as_deref().filter(|_| !listing_direct_draft) {
            if let Some(client) = sqlx::query_as::<_, SignerRow>(
                r#"
                select p.id::text as person_id,
                       p.display_name,
                       (
                         select pi.identity_value
                         from person_identity pi
                         where pi.person_id = p.id and pi.identity_type = 'email'
                         order by pi.is_primary desc, pi.created_at asc
                         limit 1
                       ) as email,
                       'BUYER'::text as role
                from deal d
                join person p on p.id = d.client_person_id
                where d.id = $1::uuid
                limit 1
                "#,
            )
            .bind(deal_id)
            .fetch_optional(&mut *self.db.connection().await?)
            .await
            .map_err(|error| DbFailure::from_sqlx("form.signers.deal_client", &error))?
            {
                people.push(FormSignerPerson {
                                slot_id: None,
                    person_id: client.person_id,
                    name: client.display_name,
                    email: compact(client.email),
                    role: client.role,
                });
            }

            let participants = sqlx::query_as::<_, SignerRow>(
                r#"
                select fp.person_id::text as person_id,
                       fp.display_name,
                       (
                         select pi.identity_value
                         from person_identity pi
                         where pi.person_id = fp.person_id and pi.identity_type = 'email'
                         order by pi.is_primary desc, pi.created_at asc
                         limit 1
                       ) as email,
                       fp.role
                from document_form_participant fp
                where fp.form_instance_id = $1::uuid
                order by fp.sort_order asc, fp.display_name
                "#,
            )
            .bind(form_instance_id)
            .fetch_all(&mut *self.db.connection().await?)
            .await
            .map_err(|error| DbFailure::from_sqlx("form.signers.form_participants", &error))?;

            for row in participants {
                people.push(FormSignerPerson {
                                slot_id: None,
                    person_id: row.person_id,
                    name: row.display_name,
                    email: compact(row.email),
                    role: role_for_form(&form.template_id, &row.role),
                });
            }

            let deal_people = sqlx::query_as::<_, SignerRow>(
                r#"
                select dp.person_id::text as person_id,
                       coalesce(person.display_name, dp.role) as display_name,
                       (
                         select pi.identity_value
                         from person_identity pi
                         where pi.person_id = dp.person_id and pi.identity_type = 'email'
                         order by pi.is_primary desc, pi.created_at asc
                         limit 1
                       ) as email,
                       dp.role
                from deal_participant dp
                left join person on person.id = dp.person_id
                where dp.deal_id = $1::uuid and dp.active = true
                order by dp.created_at asc
                "#,
            )
            .bind(deal_id)
            .fetch_all(&mut *self.db.connection().await?)
            .await
            .map_err(|error| DbFailure::from_sqlx("form.signers.deal_participants", &error))?;

            for row in deal_people {
                people.push(FormSignerPerson {
                                slot_id: None,
                    person_id: row.person_id,
                    name: row.display_name,
                    email: compact(row.email),
                    role: role_for_form(&form.template_id, &row.role),
                });
            }
        }

        if matches!(form.template_id.as_str(), "LISTING-01" | "PR-PNS") {
            let brokers = sqlx::query_as::<_, BrokerRow>(
                r#"
                select person_id::text as person_id, email
                from app_user
                where active = true and lower(display_name) = lower($1)
                order by id
                limit 2
                "#,
            )
            .bind(SELLER_BROKER_NAME)
            .fetch_all(&mut *self.db.connection().await?)
            .await
            .map_err(|error| DbFailure::from_sqlx("form.signers.broker", &error))?;

            if brokers.len() != 1 {
                return Err(DbFailure::schema_mismatch(
                    "form.signers.broker",
                    format!(
                        "{} signer resolution requires exactly one active {SELLER_BROKER_NAME} app user",
                        form.template_id
                    ),
                ));
            }
            let broker = brokers.into_iter().next().expect("checked len == 1");
            people.push(FormSignerPerson {
                            slot_id: None,
                person_id: broker.person_id,
                name: SELLER_BROKER_NAME.into(),
                email: compact(broker.email),
                role: "SELLER_BROKER".into(),
            });
        }

        Ok(people)
    }
}
