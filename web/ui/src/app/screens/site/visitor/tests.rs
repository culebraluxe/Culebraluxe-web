// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;
use crate::app::cmd::Cmd;
use crate::app::screen::ScreenCtx;
use serde_json::json;

fn listing(id: &str) -> serde_json::Value {
    json!({ "id": id, "slug": format!("{id}-slug"), "name": format!("Villa {id}"), "priceValue": 1_000_000 })
}

fn loaded(needs: Needs) -> Model {
    let (mut model, cmd) = init("site-buyers", None, &ScreenCtx::default(), needs);
    let request = cmd.into_requests().remove(0);
    assert_eq!(request.path, "/api/rust-ui/public-page?screen=site-buyers");
    update(
        &mut model,
        request.respond(Ok(json!({ "listings": [listing("a"), listing("b")] }))),
    );
    model
}

fn written(cmd: Cmd<Msg>) -> Option<(String, String)> {
    match cmd {
        Cmd::StorageWrite { key, value } => Some((key, value.unwrap_or_default())),
        Cmd::Batch(cmds) => cmds.into_iter().find_map(written),
        _ => None,
    }
}

fn property_model() -> Model {
    use crate::model::{PageContent, PropertyRecord};
    let record = PropertyRecord {
        id: "casa-luar".into(),
        title: "Casa Luar".into(),
        latitude: Some(18.315_573),
        longitude: Some(-65.255_65),
        ..PropertyRecord::default()
    };
    let mut model = Model::default();
    let mut page = PageContent::default();
    page.property = Some(record);
    model.page = Some(page);
    model
}

fn init_map(cmd: &Cmd<Msg>) -> Option<(f64, f64)> {
    match cmd {
        Cmd::InitMap { lat, lng, .. } => Some((*lat, *lng)),
        Cmd::Batch(cmds) => cmds.iter().find_map(init_map),
        _ => None,
    }
}

#[test]
fn map_mounts_once_per_record_and_ignores_stale_answers() {
    let mut model = property_model();
    // The Map tab is the one that draws the container the mount waits for.
    model.property_media.tab = PropertyTab::Map;
    // No key yet: nothing mounts.
    let cmd = update(
        &mut model,
        Msg::MapKeyLoaded(Ok(MapsKeyAnswer { key: None })),
    );
    assert!(init_map(&cmd).is_none());
    assert_eq!(model.map_mounted_for, None);

    // Key arrives: mounts Casa Luar.
    let cmd = update(
        &mut model,
        Msg::MapKeyLoaded(Ok(MapsKeyAnswer {
            key: Some("key".into()),
        })),
    );
    assert_eq!(init_map(&cmd), Some((18.315_573, -65.255_65)));
    assert_eq!(model.map_mounted_for.as_deref(), Some("casa-luar"));

    // Ready for another record is stale: owns no overlay.
    update(&mut model, Msg::MapReady("other".into()));
    assert_eq!(model.map_live_for, None);

    // Ready for Casa Luar lifts the overlay.
    update(&mut model, Msg::MapReady("casa-luar".into()));
    assert_eq!(model.map_live_for.as_deref(), Some("casa-luar"));

    // A stale failure must not kill the live map.
    update(&mut model, Msg::MapFailed("other".into()));
    assert!(!model.map_dead);
    assert_eq!(model.map_live_for.as_deref(), Some("casa-luar"));
}

#[test]
fn map_failure_is_a_state_not_a_loop() {
    let mut model = property_model();
    model.property_media.tab = PropertyTab::Map;
    update(
        &mut model,
        Msg::MapKeyLoaded(Ok(MapsKeyAnswer {
            key: Some("key".into()),
        })),
    );
    update(&mut model, Msg::MapFailed("casa-luar".into()));
    assert!(model.map_dead);
    // Tabbing back must not re-issue into the failure.
    let cmd = update(&mut model, Msg::PropertyTabSelected(PropertyTab::Map));
    assert!(init_map(&cmd).is_none());
}

#[test]
fn a_page_load_on_another_tab_does_not_burn_the_map() {
    use crate::model::PropertyRecord;
    let mut model = Model::default();
    model.map_key = Remote::Loaded(Some("key".into()));
    let mut page = PageContent::default();
    page.property = Some(PropertyRecord {
        id: "casa-luar".into(),
        latitude: Some(18.315_573),
        longitude: Some(-65.255_65),
        ..PropertyRecord::default()
    });
    // The page arrives with Overview showing, so the container the mount
    // waits for is not in the DOM: nothing may be mounted here, because a
    // mount that finds no container fails and leaves the map dead for the
    // rest of the visit.
    let cmd = update(&mut model, Msg::Loaded(Ok(page)));
    assert!(init_map(&cmd).is_none());
    assert_eq!(model.map_mounted_for, None);
    assert!(!model.map_dead);
    assert_eq!(model.map_key, Remote::Loaded(Some("key".into())));

    // The Map tab draws that container: that is where the mount goes out.
    let cmd = update(&mut model, Msg::PropertyTabSelected(PropertyTab::Map));
    assert_eq!(init_map(&cmd), Some((18.315_573, -65.255_65)));
    assert_eq!(model.map_mounted_for.as_deref(), Some("casa-luar"));
}

#[test]
fn saving_keeps_the_stores_other_entries_and_its_shape() {
    let mut model = loaded(Needs {
        favorites: true,
        buyer_tools: false,
    });
    update(
        &mut model,
        Msg::FavoritesRead(Some(
            r#"["legacy-id", {"id":"x","slug":"x","name":"X"}]"#.into(),
        )),
    );
    assert_eq!(model.saved_listings, ["legacy-id", "x"]);
    let (key, value) =
        written(update(&mut model, Msg::ListingFavoriteToggled("a".into()))).unwrap();
    assert_eq!(key, FAVORITES_KEY);
    let stored: serde_json::Value = serde_json::from_str(&value).unwrap();
    assert_eq!(
        stored,
        json!(["legacy-id", {"id":"x","slug":"x","name":"X"}, {"id":"a","slug":"a-slug","name":"Villa a"}])
    );
    assert!(model.saved_listings.contains(&"a".to_string()));
    written(update(&mut model, Msg::ListingFavoriteToggled("a".into()))).unwrap();
    assert!(!model.saved_listings.contains(&"a".to_string()));
    assert!(
        written(update(
            &mut model,
            Msg::ListingFavoriteToggled("not-on-page".into())
        ))
        .is_none(),
        "an id the page does not show is never saved"
    );
}

#[test]
fn a_stored_compare_set_is_pruned_to_listings_still_published() {
    let mut model = loaded(Needs {
        favorites: true,
        buyer_tools: true,
    });
    let stored = r#"[{"id":"a","slug":"a-slug","name":"Villa a"},{"id":"gone","slug":"gone","name":"Gone"}]"#;
    let (key, value) = written(update(&mut model, Msg::CompareRead(Some(stored.into())))).unwrap();
    assert_eq!(key, COMPARE_KEY);
    assert!(!value.contains("gone"));
    assert_eq!(model.compare.len(), 1);
}

#[test]
fn an_enquiry_is_sent_once_and_its_answer_decides_the_thank_you() {
    let mut model = loaded(Needs {
        favorites: false,
        buyer_tools: false,
    });
    let submission = ContactSubmission {
        name: "Ada".into(),
        email: "ada@example.com".into(),
        ..Default::default()
    };
    let request = update(
        &mut model,
        Msg::ContactSubmitted {
            submission: submission.clone(),
            new_id: "id-1".into(),
        },
    )
    .into_requests()
    .remove(0);
    let body = request.body.clone().unwrap();
    assert_eq!(
        (body["submissionId"].as_str(), body["requestType"].as_str()),
        (Some("id-1"), Some("general_enquiry"))
    );
    assert!(update(
        &mut model,
        Msg::ContactSubmitted {
            submission: submission.clone(),
            new_id: "id-2".into()
        }
    )
    .into_requests()
    .is_empty());
    update(
        &mut model,
        request.respond(Ok(json!({ "accepted": false }))),
    );
    assert_eq!(model.contact_form.status, ContactStatus::Failed);
    // A retry keeps the first id, so the pipeline sees one enquiry.
    let retry = update(
        &mut model,
        Msg::ContactSubmitted {
            submission,
            new_id: "id-3".into(),
        },
    )
    .into_requests()
    .remove(0);
    assert_eq!(retry.body.clone().unwrap()["submissionId"], "id-1");
    update(&mut model, retry.respond(Ok(json!({ "accepted": true }))));
    assert_eq!(model.contact_form.status, ContactStatus::Sent);
}

#[test]
fn a_property_page_records_itself_in_recently_viewed() {
    let (mut model, cmd) = init(
        "site-property-detail",
        Some("casa".into()),
        &ScreenCtx::default(),
        Needs {
            favorites: true,
            buyer_tools: false,
        },
    );
    let request = cmd.into_requests().remove(0);
    assert_eq!(
        request.path,
        "/api/rust-ui/public-page?screen=site-property-detail&scope=casa"
    );
    update(&mut model, request.respond(Ok(json!({ "property": { "id": "p1", "slug": "casa", "title": "Casa", "publicSlugs": ["casa", "villa"] } }))));
    let stored = r#"[{"slug":"villa","id":"p2","name":"Villa","at":1},{"slug":"delisted","id":"p3","name":"Old","at":2}]"#;
    let (key, value) = written(update(
        &mut model,
        Msg::RecentRead {
            raw: Some(stored.into()),
            now: 9,
        },
    ))
    .unwrap();
    assert_eq!(key, RECENT_KEY);
    let recent: Vec<PropertyRecent> = serde_json::from_str(&value).unwrap();
    assert_eq!(
        recent
            .iter()
            .map(|entry| entry.slug.as_str())
            .collect::<Vec<_>>(),
        ["casa", "villa"]
    );
    assert_eq!(
        model.property_media.recent.len(),
        1,
        "the page shows the others"
    );
}
