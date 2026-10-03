//! THE PUBLIC SITE'S COPY, and the two listing helpers the Buyers screen shares with it.
//!
//! These lived in `view.rs`, the old global loop's HTML-string renderer. That loop is deleted (2026-09-28); the words
//! and the ordering rule are the owner's and are kept here unchanged, for the site screens to render.

use std::cmp::Ordering;

use crate::model::Listing;
use crate::search::listing_price;

/// The Buyers inventory, narrowed and ordered: `applySearchFilters` from `lib/search-contract.ts`, in Rust.
///
/// THE CONTRACT IS THE SPECIFICATION, NOT MY READING OF THE SCREEN. Category, free text, a price ceiling, a bedroom
/// floor and four orderings are the canonical search surface (PX-23/PX-24) — the same rules the server applies in SQL
/// and the same ones the saved-search matcher counts alerts with. Written from that file so the three cannot drift:
///
///   - `land` keeps only land, and `homes` keeps only what is not land;
///   - a price ceiling excludes a listing with no price at all, because "unknown" is not "cheap";
///   - a bedroom floor excludes LAND whatever it says, because bedrooms on a parcel are a question with no answer —
///     the contract's own rule, and the reason the bar disables that control on the Land tab;
///   - free text matches the name, the location and the property type, case-insensitively.
///
/// The view filter is live: the payload carries each listing's `views`, and the matcher checks membership.
///
/// It takes the model rather than a filter set so the list on screen cannot be narrowed by something no control
/// explains: what is rendered is always a function of what the model holds.
pub(crate) fn buyers_visible<'a>(
    listings: &'a [Listing],
    controls: &crate::model::Controls,
) -> Vec<&'a Listing> {
    // ONE MATCHER: the grid, the saved-search counts and their alerts all ask `search::matches`.
    let filters = crate::search::SearchFilters::from_controls(controls);
    let sort = filters.sort.as_str();
    let mut visible: Vec<&Listing> = listings
        .iter()
        .filter(|listing| crate::search::matches(listing, &filters))
        .collect();

    // No price sorts last under every ordering, which is what the contract's `?? -1` and `?? MAX_SAFE_INTEGER` say.
    let for_high = |listing: &&Listing| listing_price(listing).unwrap_or(-1.0);
    visible.sort_by(|a, b| match sort {
        "price-high" => for_high(b)
            .partial_cmp(&for_high(a))
            .unwrap_or(Ordering::Equal),
        "price-low" => {
            let for_low = |listing: &&Listing| listing_price(listing).unwrap_or(f64::MAX);
            for_low(a)
                .partial_cmp(&for_low(b))
                .unwrap_or(Ordering::Equal)
        }
        // Case-insensitive: the contract compares with `localeCompare`, and comparing raw bytes would file every
        // capitalised name under a different letter than a lowercase one. Accents order by codepoint rather than by
        // collation, which is the one place this can disagree with the live order.
        "name" => a
            .name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase()),
        // `featured` is the default: featured first, then price high to low.
        _ => {
            if a.featured != b.featured {
                return if a.featured {
                    Ordering::Less
                } else {
                    Ordering::Greater
                };
            }
            for_high(b)
                .partial_cmp(&for_high(a))
                .unwrap_or(Ordering::Equal)
        }
    });
    visible
}

pub(crate) const BUYER_SERVICES: [&str; 6] = [
    "Private, unlisted viewings",
    "Legal, title & closing guidance",
    "Architecture & renovation introductions",
    "Residency & relocation support",
    "Property management referrals",
    "Long-term stewardship advice",
];

pub(crate) const BUYER_STEPS: [(&str, &str, &str); 4] = [
    ("01", "A quiet conversation", "We begin by understanding what you are truly seeking — the light, the outlook, the rhythm of days. No pressure, no listings sheet. Just a considered discussion of possibility."),
    ("02", "Private viewings", "Many of the finest homes on Culebra never reach a public listing. We arrange discreet, unhurried viewings — including off-market residences held within our private network."),
    ("03", "Diligence & title", "We coordinate title research, survey review, and legal counsel, translating the particulars of Puerto Rico property law into clear, unhurried guidance."),
    ("04", "Closing & beyond", "From closing logistics to introductions for architects, builders, and island life, we remain a steady presence well after the keys change hands."),
];

/// The eight services, in the order the page lists them, with their own images and enquiry links.
///
/// A TABLE RATHER THAN EIGHT BLOCKS OF MARKUP, because the page renders them from an array and the array is the thing a
/// reader needs to check against the live site: number, title, body, call to action, destination, image — in that order.
pub(crate) const SERVICES: [(&str, &str, &str, &str, &str, &str); 8] = [
    (
        "01",
        "Market Analysis / CMA",
        "Comprehensive market data and local insight to help you understand current value and position with confidence.",
        "Request analysis",
        "/contact?service=market-analysis",
        "/images/services/service-01-market-analysis-cma.jpg",
    ),
    (
        "02",
        "Property Evaluation",
        "A considered evaluation of your home or land based on property characteristics, location, and current market conditions.",
        "Request evaluation",
        "/contact?service=property-evaluation",
        "/images/services/service-02-property-evaluation.jpg",
    ),
    (
        "03",
        "Comparable Research",
        "Detailed comparable-property research to support informed decisions when buying, selling, or evaluating an opportunity.",
        "Request comparables",
        "/contact?service=comparable-research",
        "/images/services/service-03-comparable-research.jpg",
    ),
    (
        "04",
        "Land Survey Coordination",
        "Coordination with trusted local professionals for surveys, boundary work, and related property documentation.",
        "Request survey",
        "/contact?service=land-survey",
        "/images/services/service-04-land-survey-coordination.jpg",
    ),
    (
        "05",
        "Appraisal Coordination",
        "Assistance arranging professional appraisal services for lending, estate planning, investment, or personal decision-making.",
        "Request appraisal",
        "/contact?service=appraisal",
        "/images/services/service-05-appraisal-coordination.jpg",
    ),
    (
        "06",
        "Deed & Title Research",
        "Coordination of title history, deed research, lien checks, and document retrieval with the appropriate local professionals.",
        "Request research",
        "/contact?service=title-research",
        "/images/services/service-06-deed-title-research.jpg",
    ),
    (
        "07",
        "Real Estate Consultation",
        "Personalized guidance for property ownership, purchases, sales, investment questions, and long-range planning on Culebra.",
        "Book consultation",
        "/contact?service=consultation",
        "/images/services/service-07-real-estate-consultation.jpg",
    ),
    (
        "08",
        "Property Marketing Services",
        "Discreet, elevated property presentation and marketing support designed around the property, audience, and objective.",
        "Discuss marketing",
        "/contact?service=property-marketing",
        "/images/services/service-08-property-marketing-services.jpg",
    ),
];

pub(crate) const SERVICE_PRINCIPLES: [(&str, &str, &str); 3] = [
    ("file-search", "Research before action", "Decisions begin with understanding the property, context, documentation, and objective."),
    ("handshake", "The right people", "We help connect each need with appropriate local expertise rather than treating every request the same."),
    ("check-circle-2", "Follow-through", "Thoughtful coordination and clear communication keep small details from becoming large problems."),
];

pub(crate) const SERVICE_PROCESS: [(&str, &str, &str, &str); 3] = [
    (
        "1",
        "Share your needs",
        "Tell us about the property, your objectives, and the support you are looking for.",
        "message-circle",
    ),
    (
        "2",
        "We review & coordinate",
        "We research the situation, connect the right professionals, and organize the details.",
        "map-pinned",
    ),
    (
        "3",
        "Clear next steps",
        "You receive thoughtful guidance, timely updates, and a clear path forward.",
        "clipboard-check",
    ),
];

pub(crate) const SERVICE_REASONS: [(&str, &str, &str); 4] = [
    ("Island-specific knowledge", "Deep understanding of Culebra's properties, neighborhoods, infrastructure, market, and way of life.", "compass"),
    ("Personally handled", "Thoughtful, attentive service with direct involvement rather than a high-volume handoff model.", "user-round"),
    ("Trusted local network", "Established relationships with surveyors, attorneys, appraisers, contractors, and other island professionals.", "network"),
    ("Boutique service", "Selective client relationships, careful coordination, and discreet high-touch support.", "handshake"),
];

pub(crate) const ABOUT_CREDENTIALS: [&str; 4] = [
    "Former Windsurfing World Champion",
    "Licensed Puerto Rico Real Estate Broker",
    "Full-time Culebra resident",
    "Referral-led, boutique practice",
];

pub(crate) const ABOUT_LIFE: [&str; 8] = [
    "/images/about/life-01.jpg",
    "/images/about/life-02.jpg",
    "/images/about/life-03.jpg",
    "/images/about/life-04.jpg",
    "/images/about/life-05.jpg",
    "/images/about/life-06.jpg",
    "/images/about/life-07.jpg",
    "/images/about/life-08.jpg",
];

pub(crate) const ABOUT_REASONS: [(&str, &str); 4] = [
    (
        "Boutique by design",
        "We intentionally work with a limited number of clients.",
    ),
    (
        "Island-specific expertise",
        "We understand unique homes, land, waterfront, and the realities of island ownership.",
    ),
    (
        "Personally handled",
        "Every search, showing, and negotiation is handled directly and deliberately.",
    ),
    (
        "Trusted by referral",
        "Much of our work comes through personal introductions and word of mouth.",
    ),
];

pub(crate) const ABOUT_STATS: [(&str, &str); 4] = [
    ("14", "Years on island"),
    ("1", "Island, entirely"),
    ("40+", "Homes stewarded"),
    ("100%", "By referral"),
];

pub(crate) const ABOUT_VALUES: [(&str, &str, &str); 3] = [
    ("Fit over volume", "We measure success not in transactions but in fit — pairing the right stewards with the right homes.", "waves"),
    ("Local, truly", "Founded by island residents, we know Culebra beyond its coordinates — the trade winds, the tide charts, and the people who shape it.", "palmtree"),
    ("Quiet stewardship", "We protect the character that makes this place rare, advising with discretion and patience at every turn.", "leaf"),
];

/// Whether a listing is land — the same rule the card's Land badge uses, so the badge and the tabs cannot disagree.
pub(crate) fn listing_is_land(listing: &Listing) -> bool {
    crate::format::listing_is_land(listing)
}

pub(crate) const GUIDE_SECTIONS: [(&str, &str, &str, &str, &str); 9] = [
    ("beaches", "01", "BEACHES", "The edges of the island.", "From world-famous shores to quiet hidden coves, every beach in Culebra has its own character."),
    ("water", "02", "WATER", "The island from the water.", "Reefs, protected bays, open water and surrounding cays make the sea part of everyday life on Culebra."),
    ("wildlife-land", "03", "WILDLIFE & LAND", "A landscape worth protecting.", "Refuge lands, dry forest, trails and coastal habitat reveal the quieter natural side of the island."),
    ("coffee-casual", "04", "COFFEE & CASUAL", "Easy mornings and simple stops.", "Coffee, breakfast, beach kiosks and casual favorites for the unhurried rhythm of island days."),
    ("dining", "05", "DINING", "Where the island gathers.", "Waterfront tables, local seafood, pizza, tacos and relaxed evening spots across Culebra."),
    ("getting-here", "06", "GETTING HERE", "Arriving is part of the experience.", "Flights and ferry service connect Culebra with San Juan, Ceiba and Puerto Rico's main island."),
    ("getting-around", "07", "GETTING AROUND", "Small island, easy rhythm.", "Jeeps, carts and local taxis make it simple to move between town, beaches and the hills."),
    ("essentials", "08", "ISLAND ESSENTIALS", "The practical side of island life.", "Groceries, medical care, banking, hardware and everyday services that keep life on Culebra moving."),
    ("island-story", "09", "ISLAND STORY", "An island shaped by history.", "Indigenous roots, Spanish rule, military history, community resistance and conservation all shaped modern Culebra."),
];

pub(crate) const SELLER_DISTRIBUTION: [(&str, &str); 6] = [
    ("CulebraLuxe buyer relationships", "users"),
    ("Direct qualified-buyer outreach", "user-round"),
    ("Targeted email campaigns", "mail"),
    ("Puerto Rico listing channels & MLS", "network"),
    ("Major real estate platforms", "megaphone"),
    ("Broker network & referrals", "handshake"),
];

pub(crate) const SELLER_MARKET_LEFT: [(&str, &str, &str); 3] = [
    (
        "Property",
        "Home, improvements, land, views, condition.",
        "home",
    ),
    (
        "Place",
        "Micro-location, access, infrastructure, island context.",
        "map-pin",
    ),
    (
        "Market",
        "Comparable sales, competition, supply, and current conditions.",
        "bar-chart-3",
    ),
];

pub(crate) const SELLER_MARKET_RIGHT: [(&str, &str, &str); 2] = [
    (
        "Buyer",
        "Likely buyer pool, motivations, ability, and timing.",
        "user-round",
    ),
    (
        "Objectives",
        "Your goals, timing, flexibility, and desired outcome.",
        "flag",
    ),
];

pub(crate) const SELLER_PRESENTATION: [(&str, &str); 6] = [
    ("Professional photography", "camera"),
    ("Video & walkthroughs", "video"),
    ("Staging & presentation guidance", "home"),
    ("Editorial property story", "file-text"),
    ("Digital property marketing", "megaphone"),
    ("Print & marketing materials", "clipboard-check"),
];

pub(crate) const SELLER_PROCESS: [(&str, &str, &str); 6] = [
    (
        "Understand",
        "The property, circumstances, and objectives.",
        "search",
    ),
    (
        "Position",
        "Market analysis and pricing strategy.",
        "target",
    ),
    (
        "Prepare",
        "Property preparation and media production.",
        "camera",
    ),
    (
        "Launch",
        "Market introduction and targeted exposure.",
        "send",
    ),
    (
        "Represent",
        "Showings, offers, and skilled negotiation.",
        "users",
    ),
    (
        "Close",
        "Contract-to-closing coordination and follow-through.",
        "check-circle-2",
    ),
];

pub(crate) const SELLER_REPRESENTATION: [(&str, &str, &str); 6] = [
    (
        "Private showings",
        "Personally presenting the property to qualified buyers.",
        "users",
    ),
    (
        "Offers & negotiation",
        "Evaluating offers and negotiating terms that align with your goals.",
        "file-text",
    ),
    (
        "Contract progression",
        "Moving from accepted offer into the appropriate purchase-and-sale process.",
        "pen-line",
    ),
    (
        "Due diligence coordination",
        "Survey, appraisal, inspections, financing, title, and other diligence items.",
        "clipboard-check",
    ),
    (
        "Closing coordination",
        "Coordinating with attorneys, title professionals, and all parties.",
        "handshake",
    ),
    (
        "Successful close",
        "Following through until the transaction is complete.",
        "key-round",
    ),
];

pub(crate) const SELLER_WHY_US: [(&str, &str, &str); 3] = [
    ("Local intelligence", "Deep knowledge of properties, places, and local conditions that do not appear neatly in a database.", "compass"),
    ("Individual attention", "Every property receives its own positioning, strategy, presentation, and path to market.", "user-round"),
    ("Selective representation", "We maintain a limited portfolio so each listing receives meaningful focus and care.", "gem"),
];

/// The login-recovery page's own text, extracted from the TypeScript page it replaces.
///
/// NOT retyped, deliberately: this is reviewed wording (a policy Meta requires for the WhatsApp integration), and a
/// hand-copied paragraph is a paragraph that can quietly differ. `kind` is the tag it had, so the render below can put
/// it back in the same shape.
pub(crate) const LOGIN_RECOVERY_VIEW_CONTENT: [(&str, &str); 2] = [
    ("h1", "Emergency administrative access"),
    ("p", "For CulebraLuxe administrators only. This path is independent of the normal sign-in provider and is intended solely for outage recovery."),
];

/// The login-unauthorized page's own text, extracted from the TypeScript page it replaces.
///
/// NOT retyped, deliberately: this is reviewed wording (a policy Meta requires for the WhatsApp integration), and a
/// hand-copied paragraph is a paragraph that can quietly differ. `kind` is the tag it had, so the render below can put
/// it back in the same shape.
pub(crate) const LOGIN_UNAUTHORIZED_VIEW_CONTENT: [(&str, &str); 3] = [
    ("h1", "Access not authorized"),
    ("p", "This account is authenticated but is not authorized for CulebraLuxe. Accounts are provisioned by an administrator — there is no self-service sign-up or automatic access."),
    ("p", "If you believe this is a mistake, contact a CulebraLuxe administrator and provide the identity shown by your sign-in provider. Your password, secret, and provider credentials are never shared or displayed here."),
];

/// The site-privacy page's own text, extracted from the TypeScript page it replaces.
///
/// NOT retyped, deliberately: this is reviewed wording (a policy Meta requires for the WhatsApp integration), and a
/// hand-copied paragraph is a paragraph that can quietly differ. `kind` is the tag it had, so the render below can put
/// it back in the same shape.
pub(crate) const PRIVACY_VIEW_CONTENT: [(&str, &str); 29] = [
    ("p", "CulebraLuxe LLC"),
    ("h1", "Privacy Policy"),
    ("p", "Last updated: September 1, 2026"),
    ("h2", "Overview"),
    ("p", "CulebraLuxe LLC respects your privacy. This Privacy Policy explains how we collect, use, store, and protect information when you use our website, communicate with us, or interact with services and integrations operated by CulebraLuxe, including WhatsApp and Meta services."),
    ("h2", "Information we collect"),
    ("p", "We may collect information you provide directly to us, such as your name, email address, phone number, property information, communication preferences, and other information you choose to provide when contacting CulebraLuxe or using our services."),
    ("p", "When you communicate with CulebraLuxe through WhatsApp or other messaging services, we may receive information associated with those communications, including identifiers, phone numbers, timestamps, message status information, and message content when required to provide the requested communication service."),
    ("h2", "Meta and WhatsApp integrations"),
    ("p", "CulebraLuxe may use Meta Platforms, Inc. services, including Facebook Login for Business, the WhatsApp Business Platform, and WhatsApp Business App coexistence features. When you authorize or interact with these services, Meta may provide CulebraLuxe with information necessary to operate the integration, such as business account identifiers, WhatsApp Business Account information, phone number identifiers, access authorization information, webhook events, and messaging data associated with CulebraLuxe communications."),
    ("p", "We use this information only to operate CulebraLuxe business communications, maintain our customer and relationship records, provide requested services, troubleshoot integrations, and comply with applicable legal or platform requirements."),
    ("h2", "How we use information"),
    ("p", "We may use collected information to:"),
    ("li", "respond to inquiries and communicate with clients and prospective clients;"),
    ("li", "provide real estate brokerage and related services;"),
    ("li", "maintain client, property, transaction, and relationship records;"),
    ("li", "operate and improve our website, internal systems, and messaging integrations;"),
    ("li", "protect against fraud, misuse, security incidents, or unauthorized access; and"),
    ("li", "comply with legal, regulatory, contractual, and platform obligations."),
    ("h2", "Sharing of information"),
    ("p", "We do not sell personal information. We may share information with service providers and technology platforms only as needed to operate CulebraLuxe services, including hosting, communications, document, authentication, and messaging services. We may also disclose information when required by law or when reasonably necessary to protect CulebraLuxe, our clients, or others."),
    ("h2", "Data retention and security"),
    ("p", "We retain information only for as long as reasonably necessary for the purposes described in this policy, for legitimate business and recordkeeping needs, and as required by law. We use reasonable administrative, technical, and organizational safeguards designed to protect information from unauthorized access, loss, misuse, or disclosure."),
    ("h2", "Your choices and requests"),
    ("p", "You may contact us to ask about personal information associated with you, request a correction, or request deletion where applicable. Some information may be retained when required for legal, regulatory, transaction-record, security, or legitimate business purposes."),
    ("h2", "Third-party services"),
    ("p", "Our services may interact with third-party platforms such as Meta and WhatsApp. Those services operate under their own privacy policies and terms. CulebraLuxe is not responsible for the privacy practices of third-party services except for our own collection and use of information received through them."),
    ("h2", "Contact us"),
    ("p", "Questions or privacy requests may be sent to CulebraLuxe LLC through our public contact page at https://www.culebraluxe.com/contact."),
];
