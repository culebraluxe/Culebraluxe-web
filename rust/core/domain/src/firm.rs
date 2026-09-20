use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Firm {
    pub id: String,
    pub name: String,
    pub legal_name: Option<String>,
    pub kind: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum FieldPatch<T> {
    #[default]
    Unchanged,
    Set(Option<T>),
}

impl<T: Clone> FieldPatch<T> {
    pub fn apply(&self, current: Option<T>) -> Option<T> {
        match self {
            Self::Unchanged => current,
            Self::Set(value) => value.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsertFirmRequest {
    pub firm_id: Option<String>,
    pub name: String,
    pub legal_name: FieldPatch<String>,
    pub kind: FieldPatch<String>,
    pub status: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_patch_preserves_omitted_vs_explicit_null() {
        assert_eq!(
            FieldPatch::<String>::Unchanged.apply(Some("existing".into())),
            Some("existing".into())
        );
        assert_eq!(
            FieldPatch::<String>::Set(None).apply(Some("existing".into())),
            None
        );
    }
}
