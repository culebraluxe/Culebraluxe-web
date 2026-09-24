//! The Buyers search surface, Compare and Saved Searches: `lib/search-contract.ts`, `lib/compare.ts` and
//! `lib/saved-searches.ts`, in Rust.
//!
//! ONE MATCHER. The inventory grid, the saved-search match counts and their "new" alerts all ask `matches`, so the
//! number beside a saved search is the number of cards the same filters draw. It is the contract's rule set: category
//! by type, a price ceiling that excludes the unpriced, a bedroom floor that excludes land, view membership, and free
//! text over the name, place, type and views.
//!
//! THE STORAGE SHAPES ARE THE TYPESCRIPT ONES, field for field (`culebraluxe:compare-properties`,
//! `culebraluxe:saved-searches`), so what a visitor saved on the React site is still there on this one.
//!
//! Pure, like `format`: no DOM, so `cargo test -p ui` proves it on the host.

use crate::format::listing_is_land;
use crate::model::{Controls, Listing};

/// Compare holds at most three: the table is read across, and a fourth column stops being a comparison.
pub const COMPARE_MAX: usize = 3;

/// One property in the compare set: the canonical id plus what is needed to prune it and name it.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct CompareEntry {
    pub id: String,
    pub slug: String,
    pub name: String,
}

/// The canonical search filters, as `SearchFilters` in TypeScript: strings, with "" meaning "no filter".
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFilters {
    /// `all`, `homes` or `land`.
    pub category: String,
    pub q: String,
    pub max_price: String,
    pub beds: String,
    pub view: String,
    /// `featured`, `price-high`, `price-low` or `name`.
    pub sort: String,
}

/// A saved search: its filters, and the ids it had matched when the visitor last looked, which is what "new" is
/// measured against.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSearch {
    pub id: String,
    pub name: String,
    pub filters: SearchFilters,
    pub created_at: String,
    pub last_checked_at: Option<String>,
    pub last_match_ids: Vec<String>,
}

impl SearchFilters {
    /// What the Buyers controls say right now.
    pub fn from_controls(controls: &Controls) -> Self {
        let named = |key: &str| controls.named.get(key).cloned().unwrap_or_default();
        let sort = named("sort");
        Self {
            category: controls
                .tab
                .clone()
                .filter(|tab| !tab.is_empty())
                .unwrap_or_else(|| "all".into()),
            q: controls.query.clone(),
            max_price: named("price"),
            beds: named("beds"),
            view: named("view"),
            sort: if sort.is_empty() {
                "featured".into()
            } else {
                sort
            },
        }
    }

    /// Put these filters on the Buyers controls: applying a saved search is exactly choosing them by hand.
    pub fn apply_to(&self, controls: &mut Controls) {
        controls.tab = Some(if self.category.is_empty() {
            "all".into()
        } else {
            self.category.clone()
        });
        controls.query = self.q.clone();
        for (key, value) in [
            ("price", &self.max_price),
            ("beds", &self.beds),
            ("view", &self.view),
        ] {
            if value.trim().is_empty() {
                controls.named.remove(key);
            } else {
                controls.named.insert(key.into(), value.trim().to_string());
            }
        }
        if self.sort.is_empty() || self.sort == "featured" {
            controls.named.remove("sort");
        } else {
            controls.named.insert("sort".into(), self.sort.clone());
        }
        controls.page = 0;
    }

    /// The identity of a search, `searchFiltersToKey`: re-saving the same filters refreshes one entry, not two.
    pub fn key(&self) -> String {
        [
            self.category.clone(),
            self.q.trim().to_lowercase(),
            self.max_price.trim().to_string(),
            self.beds.trim().to_string(),
            self.view.trim().to_lowercase(),
            self.sort.clone(),
        ]
        .join("|")
    }

    /// A person's name for the search, from its non-default filters only (`describeSearchFilters`). Ordering is not
    /// part of what matches, so the sort never appears.
    pub fn describe(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        match self.category.as_str() {
            "homes" => parts.push("Homes & Villas".into()),
            "land" => parts.push("Land".into()),
            _ => {}
        }
        if let Ok(price) = self.max_price.trim().parse::<u64>() {
            if price > 0 {
                parts.push(format!("up to ${}", group_thousands(price)));
            }
        }
        if let Ok(beds) = self.beds.trim().parse::<f64>() {
            if beds > 0.0 {
                parts.push(format!("{beds}+ beds"));
            }
        }
        if !self.view.trim().is_empty() {
            parts.push(format!("{} view", self.view.trim()));
        }
        if !self.q.trim().is_empty() {
            parts.push(format!("\u{201c}{}\u{201d}", self.q.trim()));
        }
        if parts.is_empty() {
            "All properties".into()
        } else {
            parts.join(" \u{00b7} ")
        }
    }
}

fn group_thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

/// A listing's price as a number, from the formatted string the payload carries: the digits ARE the price, and a
/// string with none ("Price Upon Request") is a listing with no price.
pub fn listing_price(listing: &Listing) -> Option<f64> {
    let digits: String = listing
        .price
        .as_deref()
        .unwrap_or("")
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// Whether a listing meets the filters: `matchesSearchFilters`.
pub fn matches(listing: &Listing, filters: &SearchFilters) -> bool {
    let is_land = listing_is_land(listing);
    match filters.category.as_str() {
        "land" if !is_land => return false,
        "homes" if is_land => return false,
        _ => {}
    }
    if let Ok(ceiling) = filters.max_price.trim().parse::<f64>() {
        // "Unknown" is not "cheap": a ceiling excludes a listing with no price.
        if listing_price(listing).map_or(true, |price| price > ceiling) {
            return false;
        }
    }
    if let Ok(floor) = filters.beds.trim().parse::<f64>() {
        // Bedrooms on a parcel are a question with no answer, so a bedroom floor excludes land.
        if is_land || listing.beds.map_or(true, |beds| beds < floor) {
            return false;
        }
    }
    let view = filters.view.trim();
    if !view.is_empty()
        && !listing
            .views
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(view))
    {
        return false;
    }
    let query = filters.q.trim().to_lowercase();
    if !query.is_empty() {
        let haystack = [
            listing.name.as_str(),
            listing.location.as_deref().unwrap_or(""),
            listing.kind.as_deref().unwrap_or(""),
            &listing.views.join(" "),
        ]
        .join(" ")
        .to_lowercase();
        if !haystack.contains(&query) {
            return false;
        }
    }
    true
}

/// The ids of the listings the filters match, in inventory order.
pub fn match_ids(listings: &[Listing], filters: &SearchFilters) -> Vec<String> {
    listings
        .iter()
        .filter(|listing| matches(listing, filters))
        .map(|listing| listing.id.clone())
        .collect()
}

/// The alert: current matches the search was not looking at when the visitor last viewed it.
pub fn new_match_ids(search: &SavedSearch, current: &[String]) -> Vec<String> {
    current
        .iter()
        .filter(|id| !search.last_match_ids.contains(id))
        .cloned()
        .collect()
}

/// The views the inventory actually has, in first-seen order: the View filter offers these and nothing it could not
/// match.
pub fn view_options(listings: &[Listing]) -> Vec<String> {
    let mut options: Vec<String> = Vec::new();
    for view in listings.iter().flat_map(|listing| listing.views.iter()) {
        let view = view.trim();
        if !view.is_empty() && !options.iter().any(|known| known.eq_ignore_ascii_case(view)) {
            options.push(view.to_string());
        }
    }
    options
}

/// Toggle a listing in the compare set. Adding past `COMPARE_MAX` is refused rather than dropping another entry.
pub fn toggle_compare(entries: &[CompareEntry], listing: &Listing) -> Vec<CompareEntry> {
    if entries.iter().any(|entry| entry.id == listing.id) {
        return entries
            .iter()
            .filter(|entry| entry.id != listing.id)
            .cloned()
            .collect();
    }
    if entries.len() >= COMPARE_MAX {
        return entries.to_vec();
    }
    let mut next = entries.to_vec();
    next.push(CompareEntry {
        id: listing.id.clone(),
        slug: listing.slug.clone(),
        name: listing.name.clone(),
    });
    next
}

/// The compare set without entries that are no longer published: a delisted property cannot hold one of three slots.
pub fn prune_compare(entries: &[CompareEntry], listings: &[Listing]) -> Vec<CompareEntry> {
    entries
        .iter()
        .filter(|entry| listings.iter().any(|listing| listing.slug == entry.slug))
        .take(COMPARE_MAX)
        .cloned()
        .collect()
}

/// Save the current search, or refresh the one with the same filters: its name, and "watching from now".
pub fn save_search(
    searches: &[SavedSearch],
    filters: &SearchFilters,
    current: Vec<String>,
    new_id: &str,
    now: &str,
) -> Vec<SavedSearch> {
    let key = filters.key();
    let mut next = searches.to_vec();
    if let Some(existing) = next.iter_mut().find(|search| search.filters.key() == key) {
        existing.name = filters.describe();
        existing.last_checked_at = Some(now.to_string());
        existing.last_match_ids = current;
    } else {
        next.push(SavedSearch {
            id: new_id.to_string(),
            name: filters.describe(),
            filters: filters.clone(),
            created_at: now.to_string(),
            last_checked_at: Some(now.to_string()),
            last_match_ids: current,
        });
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(
        id: &str,
        kind: &str,
        price: Option<&str>,
        beds: Option<f64>,
        views: &[&str],
    ) -> Listing {
        Listing {
            id: id.into(),
            slug: format!("{id}-slug"),
            name: format!("Estate {id}"),
            location: Some("Zoni, Culebra".into()),
            kind: Some(kind.into()),
            price: price.map(str::to_string),
            beds,
            views: views.iter().map(|view| view.to_string()).collect(),
            ..Listing::default()
        }
    }

    fn inventory() -> Vec<Listing> {
        vec![
            listing(
                "a",
                "Luxury Estate",
                Some("$2,500,000"),
                Some(8.0),
                &["Ocean", "Beach"],
            ),
            listing("b", "Villa", Some("$1,200,000"), Some(3.0), &["Bay"]),
            listing("c", "Land", Some("$400,000"), None, &["Ocean"]),
            listing("d", "Villa", None, Some(5.0), &[]),
        ]
    }

    fn filters(update: impl FnOnce(&mut SearchFilters)) -> SearchFilters {
        let mut filters = SearchFilters {
            category: "all".into(),
            sort: "featured".into(),
            ..Default::default()
        };
        update(&mut filters);
        filters
    }

    #[test]
    fn the_matcher_is_the_contract() {
        let all = inventory();
        assert_eq!(
            match_ids(&all, &filters(|f| f.category = "land".into())),
            vec!["c"]
        );
        assert_eq!(
            match_ids(&all, &filters(|f| f.category = "homes".into())),
            vec!["a", "b", "d"]
        );
        // A ceiling excludes the unpriced listing.
        assert_eq!(
            match_ids(&all, &filters(|f| f.max_price = "2000000".into())),
            vec!["b", "c"]
        );
        // A bedroom floor excludes land whatever it says.
        assert_eq!(
            match_ids(&all, &filters(|f| f.beds = "4".into())),
            vec!["a", "d"]
        );
        assert_eq!(
            match_ids(&all, &filters(|f| f.view = "ocean".into())),
            vec!["a", "c"]
        );
        assert_eq!(match_ids(&all, &filters(|f| f.q = "BAY".into())), vec!["b"]);
    }

    #[test]
    fn a_search_is_named_by_what_it_narrows() {
        let search = filters(|f| {
            f.category = "homes".into();
            f.max_price = "2500000".into();
            f.beds = "3".into();
            f.view = "Ocean".into();
            f.q = "zoni".into();
            f.sort = "price-low".into();
        });
        assert_eq!(search.describe(), "Homes & Villas \u{b7} up to $2,500,000 \u{b7} 3+ beds \u{b7} Ocean view \u{b7} \u{201c}zoni\u{201d}");
        assert_eq!(filters(|_| {}).describe(), "All properties");
    }

    #[test]
    fn saving_the_same_filters_refreshes_one_entry_and_new_matches_are_counted_from_the_last_look()
    {
        let all = inventory();
        let ocean = filters(|f| f.view = "Ocean".into());
        let saved = save_search(&[], &ocean, vec!["a".into()], "ss-1", "t1");
        assert_eq!(saved.len(), 1);
        // "c" appeared since the visitor last looked.
        let current = match_ids(&all, &ocean);
        assert_eq!(new_match_ids(&saved[0], &current), vec!["c"]);
        // Re-saving the same filters (case aside) is the same search, watching from now.
        let again = save_search(
            &saved,
            &filters(|f| f.view = "OCEAN".into()),
            current.clone(),
            "ss-2",
            "t2",
        );
        assert_eq!(again.len(), 1);
        assert_eq!(again[0].id, "ss-1");
        assert!(new_match_ids(&again[0], &current).is_empty());
    }

    #[test]
    fn compare_holds_three_and_refuses_a_fourth() {
        let all = inventory();
        let mut set = Vec::new();
        for listing in &all {
            set = toggle_compare(&set, listing);
        }
        assert_eq!(
            set.iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
        // Toggling a chosen one removes it; a delisted one is pruned.
        let set = toggle_compare(&set, &all[1]);
        assert_eq!(set.len(), 2);
        assert_eq!(prune_compare(&set, &all[..1]).len(), 1);
    }

    #[test]
    fn the_storage_shape_is_the_typescript_one() {
        let raw = r#"[{"id":"ss-1","name":"Land","filters":{"category":"land","q":"","maxPrice":"","beds":"","view":"","sort":"featured"},"createdAt":"t","lastCheckedAt":null,"lastMatchIds":["c"]}]"#;
        let parsed: Vec<SavedSearch> =
            serde_json::from_str(raw).expect("the React site's saved searches still read");
        assert_eq!(parsed[0].filters.category, "land");
        assert_eq!(serde_json::to_string(&parsed).unwrap(), raw);
    }

    #[test]
    fn filters_round_trip_through_the_controls() {
        let search = filters(|f| {
            f.category = "homes".into();
            f.view = "Bay".into();
            f.sort = "name".into();
        });
        let mut controls = Controls::default();
        search.apply_to(&mut controls);
        assert_eq!(SearchFilters::from_controls(&controls), search);
    }
}
