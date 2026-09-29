//! Moved from `apple_mail.rs` (move only): mail_intake, mail_promote.

#[allow(unused_imports)]
use super::*;

/// `apple-sync mail-intake` — land every configured account's mail for the requested band(s).
///
/// One account Mail.app will not answer for must not stop the others: a daily sync that dies on
/// the first account is useless. Its failure is reported and the exit code is non-zero.
pub async fn mail_intake(args: &[String]) -> Result<(), Box<dyn Error>> {
    crate::apple_sync::load_env();
    let target = target_arg(args)?;
    let accounts = configured_accounts(option(args, "--account"))?;
    let bands = bands_arg(args)?;
    let page_size = positive_int(args, "--page-size", LANDING_BATCH as i64)?.min(1000);
    let max_pages = option(args, "--max-pages")
        .map(|_| positive_int(args, "--max-pages", 1))
        .transpose()?;
    let verify = args.iter().any(|arg| arg == "--verify");
    let reset = args.iter().any(|arg| arg == "--reset-checkpoint");

    let database = connect(target).await?;
    let landing = LandingDao::new(database);

    let mut failures: Vec<String> = Vec::new();
    let mut summary = Vec::new();
    for account in &accounts {
        let account_id = match resolve_account_id(account) {
            Ok(account_id) => account_id,
            Err(error) => {
                failures.push(format!("{account}: {error}"));
                continue;
            }
        };

        if verify {
            match extract_page(account, &account_id, &bands[0], page_size, None, true) {
                Ok(page) => {
                    let mailboxes = page.mailboxes.unwrap_or(ExtractMailboxes {
                        inbox: Vec::new(),
                        sent: Vec::new(),
                    });
                    println!("Apple Mail Envelope Index verification OK");
                    println!("account={}", page.account.unwrap_or_else(|| account.clone()));
                    println!(
                        "account_id={}",
                        page.account_id.unwrap_or_else(|| account_id.clone())
                    );
                    println!("mail_version={}", page.mail_version.unwrap_or_default());
                    println!("band={} ({})", bands[0], band_label(&bands[0]));
                    println!(
                        "window={} .. {}",
                        page.since.unwrap_or_default(),
                        page.before.unwrap_or_else(|| "now".to_owned())
                    );
                    println!("inbox={}", mailboxes.inbox.join(", "));
                    println!("sent={}", mailboxes.sent.join(", "));
                    println!("sample_records={}", page.records.len());
                    println!("database_writes=0");
                }
                Err(error) => failures.push(format!("{account} verify: {error}")),
            }
            continue;
        }

        for band in &bands {
            match intake_band(
                &landing,
                account,
                &account_id,
                band,
                page_size,
                max_pages,
                reset,
            )
            .await
            {
                Ok(tally) => summary.push(json!({
                    "account": account,
                    "band": band,
                    "pages": tally.pages,
                    "records": tally.records,
                    "landed": tally.landed,
                    "replayed": tally.replayed,
                    "complete": tally.complete,
                })),
                Err(error) => {
                    failures.push(format!("{account} band {band}: {error}"));
                    break;
                }
            }
        }
    }

    println!(
        "{}",
        json!({
            "source": INTAKE_SOURCE,
            "target": target.as_str(),
            "bands": bands,
            "accounts": accounts,
            "summary": summary,
            "failures": failures,
        })
    );

    if failures.is_empty() {
        Ok(())
    } else {
        for failure in &failures {
            eprintln!("applemail FAILED {failure}");
        }
        Err(io::Error::other(format!(
            "{} account/band failure(s); the rest of the run completed",
            failures.len()
        ))
        .into())
    }
}

/// `apple-sync mail-promote` — `l_applemail` -> evidence -> interaction -> client read models.
///
/// The ONLY reader of a mail landing table. Nothing client-facing reads `l_applemail`, and this
/// pass never re-reads Mail: it works from what was landed, so the classification rules live in
/// one place (`domain::applemail`) instead of two.
pub async fn mail_promote(args: &[String]) -> Result<(), Box<dyn Error>> {
    crate::apple_sync::load_env();
    let target = target_arg(args)?;
    let days = positive_int(args, "--days", 90)?;
    let account = option(args, "--account").map(str::to_owned);
    let verify_only = args.iter().any(|arg| arg == "--verify");
    let internal = internal_addresses()?;

    let database = connect(target).await?;
    let landing = LandingDao::new(database.clone());
    let evidence_dao = RelationshipEvidenceDao::new(database);

    let rows = landing.landed_applemail(days, account.as_deref()).await?;
    let MailNormalization {
        observations,
        skipped,
    } = normalize_landed_mail(&rows, &internal);
    let accounts: BTreeSet<String> = observations
        .iter()
        .map(|row| row.source_account.clone())
        .collect();
    println!(
        "l_applemail: {} landed rows in the last {} day(s){} -> {} observations",
        rows.len(),
        days,
        account
            .as_deref()
            .map(|value| format!(" for {value}"))
            .unwrap_or_default(),
        observations.len()
    );
    println!("skipped: {}", json!(skipped));

    if verify_only {
        println!(
            "verify only — no writes. accounts={}",
            accounts.iter().cloned().collect::<Vec<_>>().join(", ")
        );
        return Ok(());
    }
    if observations.is_empty() {
        println!("nothing to promote.");
        return Ok(());
    }

    // Evidence, reconciled to a canonical Person — one row per counterparty email.
    let builds = build_mail_evidence(&observations);
    let owners = crate::apple_messages::OwnerIndex::load(&evidence_dao).await?;
    let links: HashMap<(String, String), String> = evidence_dao
        .source_links(ICLOUD_MAIL_SOURCE)
        .await?
        .into_iter()
        .map(|link| {
            (
                (link.source_account, link.source_identity_key),
                link.canonical_person_id,
            )
        })
        .collect();

    let mut tally: BTreeMap<String, i64> = BTreeMap::new();
    for row in &builds {
        let lookup = crate::apple_messages::lookup_for(row, &owners, &links);
        let decision = domain::decide_apple_handle(row, &lookup);
        *tally.entry(decision.review_state.clone()).or_insert(0) += 1;
        let id = evidence_dao
            .upsert_evidence(&db::EvidenceUpsert::from(row))
            .await?;
        evidence_dao.record_decision(&id, &decision).await?;
    }
    println!(
        "evidence: {} counterparties, reconcile tally {}",
        builds.len(),
        json!(tally)
    );

    // Only evidence already linked to a Person can become a comms event. The link is read back
    // rather than taken from the decision in memory: the row is the one writer of a link, and an
    // established link survives a later ambiguous pass (`record_decision` keeps it).
    let linked: HashMap<(String, String), String> = evidence_dao
        .candidates(Some(ICLOUD_MAIL_SOURCE), Some("exact_linked"), &[], 10000)
        .await?
        .into_iter()
        .filter(|row| accounts.contains(&row.source_account))
        .filter_map(|row| {
            row.canonical_person_id
                .map(|person_id| ((row.source_account, row.source_identity_key), person_id))
        })
        .collect();

    let mut inserted = 0i64;
    let mut replayed = 0i64;
    let mut unlinked = 0i64;
    for observation in &observations {
        let Some(person_id) = linked.get(&(
            observation.source_account.clone(),
            observation.external_email.clone(),
        )) else {
            // Evidence stays staged for the unlinked: this is a decision, not a failure.
            unlinked += 1;
            continue;
        };
        let interaction = mail_observation_interaction(observation, person_id);
        let draft = InteractionDraft {
            person_id: interaction.person_id,
            channel: interaction.channel.to_owned(),
            event_type: interaction.event_type.to_owned(),
            direction: Some(interaction.direction.to_owned()),
            occurred_at: interaction.occurred_at,
            title: interaction.title,
            source_system: interaction.source_system.to_owned(),
            source_external_id: interaction.source_external_id,
            source_metadata: interaction.source_metadata,
        };
        if landing.create_interaction(&draft).await? {
            inserted += 1;
        } else {
            replayed += 1;
        }
    }

    let refreshed = inserted > 0;
    if refreshed {
        landing.refresh_client_read_models().await?;
    }
    println!(
        "{}",
        json!({
            "source": ICLOUD_MAIL_SOURCE,
            "target": target.as_str(),
            "days": days,
            "landedRows": rows.len(),
            "observations": observations.len(),
            "skipped": skipped,
            "evidenceRows": builds.len(),
            "reconcileTally": tally,
            "interactionsInserted": inserted,
            "interactionsReplayed": replayed,
            "unlinked": unlinked,
            "refreshed": refreshed,
        })
    );
    println!(
        "promotion complete: interactions inserted={inserted} replayed={replayed} unlinked={unlinked} (evidence stays staged for the unlinked)"
    );
    Ok(())
}
