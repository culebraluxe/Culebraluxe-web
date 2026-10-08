//! L0 security boundary: delegates to the same redirect policy used by Google login and callback.

pub struct SecurityHarness;

impl SecurityHarness {
    pub fn redirect_target(next: Option<&str>) -> String {
        web::api::google_auth::safe_next(next)
    }
}
