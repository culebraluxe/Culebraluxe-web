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
