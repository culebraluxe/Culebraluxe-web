use crate::{Database, DbFailure, DbResult};
use domain::{MarketingContentBlock, MarketingContentItem};
use sqlx::FromRow;
use std::collections::BTreeMap;

#[derive(Debug, FromRow)]
struct BlockRow {
    id: String,
    kind: String,
    title: Option<String>,
    subtitle: Option<String>,
    eyebrow: Option<String>,
    body: Option<String>,
    cta_label: Option<String>,
    cta_href: Option<String>,
    image_path: Option<String>,
    image_alt: Option<String>,
}

#[derive(Debug, FromRow)]
struct ItemRow {
    content_id: String,
    item_key: String,
    label: Option<String>,
    value: Option<String>,
}

#[derive(Clone)]
pub struct MarketingDao {
    db: Database,
}

impl MarketingDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn public_content(&self) -> DbResult<Vec<MarketingContentBlock>> {
        let blocks = sqlx::query_as::<_, BlockRow>(
            r#"
            select id, kind, title, subtitle, eyebrow, body, cta_label, cta_href,
                   image_path, image_alt
              from marketing_content
             where is_active = true
             order by kind, sort_order, id
            "#,
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("marketing.public_content.blocks", &error))?;

        let items = sqlx::query_as::<_, ItemRow>(
            r#"
            select content_id, item_key, label, value
              from marketing_content_item
             where is_active = true
             order by content_id, sort_order, created_at
            "#,
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("marketing.public_content.items", &error))?;

        let mut by_content: BTreeMap<String, Vec<MarketingContentItem>> = BTreeMap::new();
        for item in items {
            by_content
                .entry(item.content_id)
                .or_default()
                .push(MarketingContentItem {
                    key: item.item_key,
                    label: item.label,
                    value: item.value,
                });
        }

        Ok(blocks
            .into_iter()
            .map(|block| MarketingContentBlock {
                id: block.id.clone(),
                kind: block.kind,
                title: block.title,
                subtitle: block.subtitle,
                eyebrow: block.eyebrow,
                body: block.body,
                cta_label: block.cta_label,
                cta_href: block.cta_href,
                image_path: block.image_path,
                image_alt: block.image_alt,
                items: by_content.remove(&block.id).unwrap_or_default(),
            })
            .collect())
    }
}
