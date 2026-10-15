//! THE VISITOR — what the interactive site pages share: the page they read, and what this device remembers for the
//! visitor (saved homes, the compare set, saved searches, recently viewed), the enquiry forms, and the property
//! gallery. One model and one reducer; Home, Buyers, Properties, Favorites, Contact and the property page are thin
//! screens over it (`pages.rs`).
//!
//! THE DEVICE STORE KEEPS THE TYPESCRIPT SHAPES, key for key (`culebraluxe:saved-properties`, ...), so what a visitor
//! saved on the old site is still here. Storage is read and written only through `Cmd`: nothing here touches the
//! browser, which is what lets every rule below be tested with `cargo test`.

use crate::app::api::{IntakeAnswer, MapsKey, MapsKeyAnswer, PublicPage, WebsiteIntake};
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::model::{
    ContactFormState, ContactStatus, ContactSubmission, Controls, PageContent, PropertyMediaState,
    PropertyRecent, PropertyTab,
};
use crate::search::{CompareEntry, SavedSearch};

pub const FAVORITES_KEY: &str = "culebraluxe:saved-properties";
pub const RECENT_KEY: &str = "culebraluxe:recently-viewed";
pub const COMPARE_KEY: &str = "culebraluxe:compare-properties";
pub const SAVED_SEARCHES_KEY: &str = "culebraluxe:saved-searches";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    /// The page, once it has arrived.
    pub page: Option<PageContent>,
    /// The page read failed: the site's interruption is shown.
    pub failed: bool,
    pub loading: bool,
    /// The record a page is about: a property's slug, or the property an enquiry is about (`?propertyId=`).
    pub scope: Option<String>,
    /// The service a Services link asked about (`?service=`).
    pub service: Option<String>,
    /// What a property link asked for (`?requestType=property_information`; a viewing otherwise).
    pub request_type: Option<String>,
    /// Saved homes, by listing id, in the order they were saved.
    pub saved_listings: Vec<String>,
    /// The store's saved entries as they are, so writing one back keeps the others exactly.
    favorites: Vec<serde_json::Value>,
    pub compare: Vec<CompareEntry>,
    pub saved_searches: Vec<SavedSearch>,
    /// The Buyers bar: search, the named dropdowns, the view.
    pub controls: Controls,
    pub contact_form: ContactFormState,
    pub property_media: PropertyMediaState,
    /// The Google Maps browser key: `Loaded(Some)` mounts the JS-API map,
    /// `Loaded(None)` is the waiting state, `Failed` the error state.
    pub map_key: Remote<Option<String>>,
    /// The record id a map mount was issued for. Tab switches unmount the
    /// container, so returning to the Map tab mounts again.
    pub map_mounted_for: Option<String>,
    /// The record id whose map reported ready: the loading overlay lifts.
    pub map_live_for: Option<String>,
    /// The map failed after mounting was attempted: drawn once, never retried
    /// on its own (a failure here is a state, as in the TypeScript original).
    pub map_dead: bool,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<PageContent, ApiError>),
    FavoritesRead(Option<String>),
    CompareRead(Option<String>),
    SearchesRead(Option<String>),
    RecentRead {
        raw: Option<String>,
        now: i64,
    },

    ListingFavoriteToggled(String),
    PropertyFavoriteToggled,
    CompareToggled(String),
    SearchSaved {
        new_id: String,
        now: String,
    },
    SavedSearchApplied {
        id: String,
        now: String,
    },
    SavedSearchRemoved(String),
    QueryChanged(String),
    FilterSelected {
        key: String,
        value: String,
    },
    TabSelected(String),

    ContactInterestChosen(String),
    ContactSubmitted {
        submission: ContactSubmission,
        new_id: String,
    },
    ContactAnswered(Result<IntakeAnswer, ApiError>),

    PropertyMediaSelected(usize),
    PropertyMediaPrevious,
    PropertyMediaNext,
    PropertyLightboxOpened(usize),
    PropertyLightboxClosed,
    PropertyLightboxMoved(i8),
    PropertyTabSelected(PropertyTab),
    MapKeyLoaded(Result<MapsKeyAnswer, ApiError>),
    MapReady(String),
    MapFailed(String),
}

/// What a page needs from the device.
#[derive(Clone, Copy)]
pub struct Needs {
    pub favorites: bool,
    pub buyer_tools: bool,
}

/// The first state and reads of a visitor page: its public page, and what it shows from the device.
pub fn init(
    screen: &'static str,
    scope: Option<String>,
    ctx: &crate::app::screen::ScreenCtx,
    needs: Needs,
) -> (Model, Cmd<Msg>) {
    let mut cmds = vec![Cmd::request(
        PublicPage {
            screen,
            scope: scope.clone(),
        },
        Msg::Loaded,
    )];
    if needs.favorites {
        cmds.push(Cmd::storage_read(FAVORITES_KEY, Msg::FavoritesRead));
    }
    if needs.buyer_tools {
        cmds.push(Cmd::storage_read(COMPARE_KEY, Msg::CompareRead));
        cmds.push(Cmd::storage_read(SAVED_SEARCHES_KEY, Msg::SearchesRead));
    }
    if screen == "site-property-detail" {
        // The map key rides with the property page only: no other visitor
        // screen draws a map, so no other screen pays for the read.
        cmds.push(Cmd::request(MapsKey, Msg::MapKeyLoaded));
    }
    (
        Model {
            loading: true,
            scope,
            service: ctx.query("service").map(str::to_owned),
            request_type: ctx.query("requestType").map(str::to_owned),
            ..Model::default()
        },
        Cmd::batch(cmds),
    )
}

/// A stored JSON array, keeping only the entries that parse as `T`: one malformed record drops that record, not the list.
fn entries<T: serde::de::DeserializeOwned>(raw: Option<&str>) -> Vec<T> {
    serde_json::from_str::<Vec<serde_json::Value>>(raw.unwrap_or("[]"))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| serde_json::from_value(entry).ok())
        .collect()
}

/// A saved entry's id: the old store kept bare ids, the new one `{ id, slug, name }`.
fn favorite_id(entry: &serde_json::Value) -> Option<&str> {
    entry
        .as_str()
        .or_else(|| entry.get("id").and_then(|id| id.as_str()))
}

fn write<T: serde::Serialize>(key: &str, value: &T) -> Cmd<Msg> {
    match serde_json::to_string(value) {
        Ok(json) => Cmd::storage_write(key, Some(json)),
        Err(_) => Cmd::none(),
    }
}

/// Save or unsave one listing, keeping every other entry exactly as stored.
fn set_favorite(model: &mut Model, id: &str, slug: &str, name: &str, saved: bool) -> Cmd<Msg> {
    model
        .favorites
        .retain(|entry| favorite_id(entry) != Some(id));
    model.saved_listings.retain(|saved_id| saved_id != id);
    if saved {
        model
            .favorites
            .push(serde_json::json!({ "id": id, "slug": slug, "name": name }));
        model.saved_listings.push(id.to_string());
    }
    if model
        .page
        .as_ref()
        .and_then(|page| page.property.as_ref())
        .is_some_and(|record| record.id == id)
    {
        model.property_media.saved = saved;
    }
    write(FAVORITES_KEY, &model.favorites)
}

/// A delisted property cannot hold one of the three compare slots: prune (and persist the pruning) once both the
/// listings and the stored set are known.
fn prune(model: &mut Model) -> Cmd<Msg> {
    let Some(page) = model.page.as_ref() else {
        return Cmd::none();
    };
    let pruned = crate::search::prune_compare(&model.compare, &page.listings);
    if pruned == model.compare {
        return Cmd::none();
    }
    model.compare = pruned;
    write(COMPARE_KEY, &model.compare)
}

/// How many pictures the property gallery steps through: the hero, then the gallery without the hero's duplicate.
fn media_total(model: &Model) -> usize {
    let Some(record) = model.page.as_ref().and_then(|page| page.property.as_ref()) else {
        return 0;
    };
    let gallery = record
        .gallery
        .iter()
        .filter_map(|item| item.src())
        .collect::<Vec<_>>();
    let Some(hero) = record.hero_url.as_deref() else {
        return gallery.len();
    };
    let duplicated = gallery.iter().any(|src| *src == hero);
    1 + gallery.len().saturating_sub(usize::from(duplicated))
}

pub fn update(model: &mut Model, msg: Msg) -> Cmd<Msg> {
    match msg {
        Msg::Loaded(answer) => {
            model.loading = false;
            match answer {
                Ok(page) => {
                    let record = page.property.as_ref().map(|record| record.id.clone());
                    if record.as_deref() != model.map_mounted_for.as_deref() {
                        // Another record (or the first): a previous mount or
                        // failure belongs to the old one, not this one.
                        model.map_mounted_for = None;
                        model.map_live_for = None;
                        model.map_dead = false;
                    }
                    model.page = Some(page);
                    model.contact_form = ContactFormState::default();
                    let mut cmds = vec![prune(model)];
                    if record.is_some_and(|id| !id.is_empty()) {
                        model.property_media = PropertyMediaState {
                            saved: model.property_media.saved,
                            ..PropertyMediaState::default()
                        };
                        cmds.push(Cmd::storage_read(RECENT_KEY, |raw| Msg::RecentRead {
                            raw,
                            now: now_ms(),
                        }));
                    }
                    cmds.push(maybe_init_map(model));
                    Cmd::batch(cmds)
                }
                Err(_) => {
                    model.failed = true;
                    Cmd::none()
                }
            }
        }
        Msg::FavoritesRead(raw) => {
            model.favorites =
                serde_json::from_str(raw.as_deref().unwrap_or("[]")).unwrap_or_default();
            model.saved_listings = model
                .favorites
                .iter()
                .filter_map(favorite_id)
                .map(str::to_owned)
                .collect();
            if let Some(record) = model.page.as_ref().and_then(|page| page.property.as_ref()) {
                model.property_media.saved = model.saved_listings.contains(&record.id);
            }
            Cmd::none()
        }
        Msg::CompareRead(raw) => {
            model.compare = entries(raw.as_deref());
            prune(model)
        }
        Msg::SearchesRead(raw) => {
            model.saved_searches = entries(raw.as_deref());
            Cmd::none()
        }
        Msg::RecentRead { raw, now } => {
            // This property goes to the front of the recently-viewed list (six at most, only properties still public),
            // and the page shows the others.
            let Some(record) = model.page.as_ref().and_then(|page| page.property.as_ref()) else {
                return Cmd::none();
            };
            let existing: Vec<PropertyRecent> =
                serde_json::from_str(raw.as_deref().unwrap_or("[]")).unwrap_or_default();
            let mut recorded = vec![PropertyRecent {
                slug: record.slug.clone(),
                id: record.id.clone(),
                name: record.title.clone(),
                at: now,
            }];
            recorded.extend(existing.into_iter().filter(|entry| entry.id != record.id));
            recorded.truncate(6);
            recorded.retain(|entry| {
                entry.slug == record.slug || record.public_slugs.contains(&entry.slug)
            });
            model.property_media.recent = recorded
                .iter()
                .filter(|entry| entry.slug != record.slug)
                .cloned()
                .collect();
            write(RECENT_KEY, &recorded)
        }

        Msg::ListingFavoriteToggled(id) => {
            // Only a listing the page shows can be saved from a card: the id is looked up, never trusted.
            let Some(listing) = model
                .page
                .as_ref()
                .and_then(|page| {
                    page.listings
                        .iter()
                        .chain(page.featured.iter())
                        .find(|listing| listing.id == id)
                })
                .cloned()
            else {
                return Cmd::none();
            };
            let saved = !model.saved_listings.contains(&listing.id);
            set_favorite(model, &listing.id, &listing.slug, &listing.name, saved)
        }
        Msg::PropertyFavoriteToggled => {
            let Some(record) = model
                .page
                .as_ref()
                .and_then(|page| page.property.as_ref())
                .cloned()
            else {
                return Cmd::none();
            };
            let saved = !model.property_media.saved;
            set_favorite(model, &record.id, &record.slug, &record.title, saved)
        }
        Msg::CompareToggled(id) => {
            let Some(listing) = model
                .page
                .as_ref()
                .and_then(|page| page.listings.iter().find(|listing| listing.id == id))
                .cloned()
            else {
                return Cmd::none();
            };
            let next = crate::search::toggle_compare(&model.compare, &listing);
            if next == model.compare {
                return Cmd::none();
            }
            model.compare = next;
            write(COMPARE_KEY, &model.compare)
        }
        Msg::SearchSaved { new_id, now } => {
            let Some(page) = model.page.as_ref() else {
                return Cmd::none();
            };
            let filters = crate::search::SearchFilters::from_controls(&model.controls);
            let current = crate::search::match_ids(&page.listings, &filters);
            model.saved_searches =
                crate::search::save_search(&model.saved_searches, &filters, current, &new_id, &now);
            write(SAVED_SEARCHES_KEY, &model.saved_searches)
        }
        Msg::SavedSearchApplied { id, now } => {
            let Some(search) = model
                .saved_searches
                .iter()
                .find(|search| search.id == id)
                .cloned()
            else {
                return Cmd::none();
            };
            let listings = model
                .page
                .as_ref()
                .map(|page| page.listings.clone())
                .unwrap_or_default();
            search.filters.apply_to(&mut model.controls);
            // Viewing it clears its alert: what it matches now becomes what it has seen.
            let current = crate::search::match_ids(&listings, &search.filters);
            if let Some(entry) = model.saved_searches.iter_mut().find(|entry| entry.id == id) {
                entry.last_match_ids = current;
                entry.last_checked_at = Some(now);
            }
            write(SAVED_SEARCHES_KEY, &model.saved_searches)
        }
        Msg::SavedSearchRemoved(id) => {
            let before = model.saved_searches.len();
            model.saved_searches.retain(|search| search.id != id);
            if model.saved_searches.len() == before {
                return Cmd::none();
            }
            write(SAVED_SEARCHES_KEY, &model.saved_searches)
        }
        Msg::QueryChanged(query) => {
            model.controls.query = query;
            model.controls.page = 0;
            Cmd::none()
        }
        Msg::FilterSelected { key, value } => {
            // The empty option is "no filter": it removes the entry, so unset is one state and not two.
            if value.is_empty() {
                model.controls.named.remove(&key);
            } else {
                model.controls.named.insert(key, value);
            }
            model.controls.page = 0;
            Cmd::none()
        }
        Msg::TabSelected(tab) => {
            model.controls.tab = Some(tab);
            model.controls.page = 0;
            Cmd::none()
        }

        Msg::ContactInterestChosen(interest) => {
            if matches!(interest.as_str(), "Buying" | "Selling" | "Both") {
                model.contact_form.interest = interest;
            }
            Cmd::none()
        }
        Msg::ContactSubmitted { submission, new_id } => {
            // One submission at a time, and none after the thank-you: a double click is one enquiry, not two. The id is
            // kept across retries, so the pipeline recognises a retry as the same enquiry.
            if matches!(
                model.contact_form.status,
                ContactStatus::Sending | ContactStatus::Sent
            ) {
                return Cmd::none();
            }
            let submission_id = model
                .contact_form
                .submission_id
                .get_or_insert(new_id)
                .clone();
            model.contact_form.status = ContactStatus::Sending;
            let mut body = serde_json::json!({
                "submissionId": submission_id,
                "requestType": if submission.request_type.is_empty() { "general_enquiry" } else { submission.request_type.as_str() },
                "name": submission.name,
                "email": submission.email,
                "message": submission.message,
                "company": submission.company,
            });
            if !submission.property_id.is_empty() {
                body["propertyId"] = serde_json::Value::String(submission.property_id);
            }
            if !submission.service.is_empty() {
                body["service"] = serde_json::Value::String(submission.service);
            }
            Cmd::request(WebsiteIntake { body }, Msg::ContactAnswered)
        }
        Msg::ContactAnswered(answer) => {
            if model.contact_form.status == ContactStatus::Sending {
                model.contact_form.status = if answer.is_ok_and(|answer| answer.accepted) {
                    ContactStatus::Sent
                } else {
                    ContactStatus::Failed
                };
            }
            Cmd::none()
        }

        Msg::PropertyMediaSelected(index) => {
            if index < media_total(model) {
                model.property_media.active_index = index;
            }
            Cmd::none()
        }
        Msg::PropertyMediaPrevious => {
            let total = media_total(model);
            if total > 1 {
                model.property_media.active_index =
                    (model.property_media.active_index + total - 1) % total;
            }
            Cmd::none()
        }
        Msg::PropertyMediaNext => {
            let total = media_total(model);
            if total > 1 {
                model.property_media.active_index = (model.property_media.active_index + 1) % total;
            }
            Cmd::none()
        }
        Msg::PropertyLightboxOpened(index) => {
            let total = media_total(model);
            if total > 0 {
                model.property_media.lightbox_index = index % total;
                model.property_media.lightbox_open = true;
            }
            Cmd::none()
        }
        Msg::PropertyLightboxClosed => {
            model.property_media.lightbox_open = false;
            Cmd::none()
        }
        Msg::PropertyLightboxMoved(delta) => {
            let total = media_total(model) as i64;
            if model.property_media.lightbox_open && total > 1 {
                let current = model.property_media.lightbox_index as i64;
                model.property_media.lightbox_index =
                    (current + i64::from(delta)).rem_euclid(total) as usize;
            }
            Cmd::none()
        }
        Msg::PropertyTabSelected(tab) => {
            model.property_media.tab = tab;
            if tab == PropertyTab::Map {
                // The tab switch unmounted the map container: mount again.
                model.map_mounted_for = None;
                maybe_init_map(model)
            } else {
                Cmd::none()
            }
        }
        Msg::MapKeyLoaded(answer) => {
            model.map_key = Remote::from_result(answer.map(|answer| answer.key));
            maybe_init_map(model)
        }
        Msg::MapReady(slug) => {
            // A superseded mount (the tab moved on mid-load) reports for a
            // record that is no longer mounted: it owns no overlay.
            if model.map_mounted_for.as_deref() == Some(slug.as_str()) {
                model.map_live_for = Some(slug);
                model.map_dead = false;
            }
            Cmd::none()
        }
        Msg::MapFailed(slug) => {
            if model.map_mounted_for.as_deref() == Some(slug.as_str()) {
                model.map_dead = true;
                model.map_mounted_for = None;
                model.map_live_for = None;
            }
            Cmd::none()
        }
    }
}

/// Mount the JS-API map when everything it needs has arrived: a configured
/// key, coordinates on the record, and no mount issued for this record yet. A
/// mount already attempted and failed stays failed — see `map_dead`.
fn maybe_init_map(model: &mut Model) -> Cmd<Msg> {
    if model.map_dead {
        return Cmd::none();
    }
    // The container is drawn by the Map tab and nowhere else
    // (`property_detail/sections.rs`), and a mount waits two seconds for it.
    // Mounting while another tab is selected therefore waits for an element that
    // will never exist, fails, and leaves `map_dead` set for the rest of the
    // visit — the map never loads, even after the visitor opens its tab. Only
    // the tab that draws the map may mount it.
    if model.property_media.tab != PropertyTab::Map {
        return Cmd::none();
    }
    let Remote::Loaded(key) = &model.map_key else {
        return Cmd::none();
    };
    let Some(key) = key.as_deref().filter(|key| !key.is_empty()) else {
        return Cmd::none();
    };
    let Some(record) = model.page.as_ref().and_then(|page| page.property.as_ref()) else {
        return Cmd::none();
    };
    let Some((lat, lng)) = record.latitude.zip(record.longitude) else {
        return Cmd::none();
    };
    if model.map_mounted_for.as_deref() == Some(record.id.as_str()) {
        return Cmd::none();
    }
    model.map_mounted_for = Some(record.id.clone());
    let slug = record.id.clone();
    Cmd::init_map(
        key,
        lat,
        lng,
        record.title.clone(),
        "property-map",
        move |result| match result {
            Ok(()) => Msg::MapReady(slug),
            Err(_) => Msg::MapFailed(slug),
        },
    )
}

/// The clock, for the recently-viewed record. Zero off the browser, where only tests run this.
fn now_ms() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as i64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0
    }
}

#[cfg(test)]
mod tests;
