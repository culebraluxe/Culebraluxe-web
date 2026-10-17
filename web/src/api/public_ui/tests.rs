// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::select_maps_key;

fn clear() {
    for key in [
        "APP_ENV",
        "VERCEL_ENV",
        "GOOGLE_MAPS_API_KEY",
        "GOOGLE_MAPS_DEMO_KEY",
    ] {
        std::env::remove_var(key);
    }
}

#[test]
fn maps_key_follows_the_same_selection_as_readiness() {
    clear();
    // Nothing configured anywhere: the page draws its waiting state.
    assert_eq!(select_maps_key(), None);

    std::env::set_var("GOOGLE_MAPS_API_KEY", "api-key");
    assert_eq!(select_maps_key().as_deref(), Some("api-key"));

    // Outside production the demo key wins when set.
    std::env::set_var("GOOGLE_MAPS_DEMO_KEY", "demo-key");
    assert_eq!(select_maps_key().as_deref(), Some("demo-key"));

    // In production the API key is the only answer, and the demo key
    // must be absent there (the readiness gate asserts the same).
    std::env::set_var("APP_ENV", "production");
    assert_eq!(select_maps_key().as_deref(), Some("api-key"));

    std::env::remove_var("GOOGLE_MAPS_API_KEY");
    assert_eq!(select_maps_key(), None);
    clear();
}

/// A film Mux holds has no uploaded filename: its media row carries `mux-<asset id>` (the repo writes that
/// identifier itself, db/src/media/media_row.rs). The property page must never show that id as the film's
/// title — the asset's identity is plumbing. A real uploaded filename still becomes the title.
#[test]
fn a_mux_films_asset_id_never_becomes_its_title() {
    let asset_id = "PDTYFQYWIR7YT016VGSUTPHEHVYECMXGCVVQWVB9WYPC";
    let property = serde_json::json!({
        "name": "Casa Culebra",
        "media": [
            {
                "id": "media-1",
                "mediaType": "video",
                "role": "video",
                "filename": format!("mux-{asset_id}"),
                "muxPlaybackId": "playback-1"
            },
            {
                "id": "media-2",
                "mediaType": "video",
                "role": "video",
                "filename": "walkthrough.mp4",
                "muxPlaybackId": "playback-2"
            }
        ]
    });

    let body = super::property_detail(
        "casa-culebra",
        &property,
        &serde_json::json!([]),
        serde_json::json!([]),
        &std::collections::HashMap::<String, super::MediaUrls>::new(),
        &std::collections::HashMap::<String, String>::new(),
    );

    assert_eq!(
        body["property"]["videos"][0]["title"],
        serde_json::Value::Null
    );
    assert_eq!(body["property"]["videos"][1]["title"], "walkthrough.mp4");
    // Not merely absent from the title: the identifier is nowhere in the page's payload.
    assert!(!body.to_string().contains(asset_id));
}
