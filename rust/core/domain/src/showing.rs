use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShowingReportOutcome {
    Interested,
    SecondShowing,
    OfferExpected,
    NotAFit,
}

impl ShowingReportOutcome {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Interested => "Interested",
            Self::SecondShowing => "Second showing",
            Self::OfferExpected => "Offer expected",
            Self::NotAFit => "Not a fit",
        }
    }
}

impl TryFrom<&str> for ShowingReportOutcome {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "Interested" => Ok(Self::Interested),
            "Second showing" => Ok(Self::SecondShowing),
            "Offer expected" => Ok(Self::OfferExpected),
            "Not a fit" => Ok(Self::NotAFit),
            other => Err(format!("unknown showing outcome: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Showing {
    pub id: String,
    pub person_id: String,
    pub property_id: String,
    pub status: String,
    pub showing_date: Option<String>,
    pub duration: Option<String>,
    pub outcome: Option<ShowingReportOutcome>,
    pub interest_score: Option<i16>,
    pub feedback: Option<String>,
    pub follow_up: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveShowingReportRequest {
    pub showing_id: String,
    pub person_id: String,
    pub property_id: String,
    pub showing_date: Option<String>,
    pub duration: Option<String>,
    pub outcome: Option<ShowingReportOutcome>,
    pub interest_score: Option<i16>,
    pub feedback: Option<String>,
    pub follow_up: Option<String>,
}
