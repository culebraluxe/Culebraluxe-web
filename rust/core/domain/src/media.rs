use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAsset {
    pub id: String,
    pub property_id: String,
    pub media_type: String,
    pub role: String,
    pub sort_order: i32,
    pub filename: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
    pub created_at: Option<String>,
    pub url: String,
}


pub const MAX_MEDIA_UPLOAD_BYTES: usize = 50 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadPropertyMediaRequest {
    pub property_id: String,
    pub role: String,
    pub filename: String,
    pub mime_type: String,
    pub alt_text: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadPropertyMediaResult {
    pub ok: bool,
    pub media_id: String,
    pub property_id: String,
    pub role: String,
}

pub fn sanitize_media_filename(name: &str) -> String {
    let base = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .trim();
    let mut cleaned = String::with_capacity(base.len().min(255));
    let mut previous_dot = false;
    for ch in base.chars() {
        if ch.is_control() {
            continue;
        }
        if ch == '.' {
            if previous_dot {
                continue;
            }
            previous_dot = true;
        } else {
            previous_dot = false;
        }
        cleaned.push(ch);
        if cleaned.len() >= 255 {
            break;
        }
    }
    if cleaned.is_empty() {
        "upload".into()
    } else {
        cleaned
    }
}
