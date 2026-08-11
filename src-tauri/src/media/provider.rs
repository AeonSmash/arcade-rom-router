//! Media provider trait for optional online scrapers (SPEC.md §42.2).

use sqlx::SqlitePool;

use crate::error::AppResult;
use crate::model::MediaAsset;

pub trait MediaProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn is_configured(&self) -> bool;
    fn fetch(
        &self,
        pool: &SqlitePool,
        archive_id: i64,
        set_name: &str,
    ) -> impl std::future::Future<Output = AppResult<Vec<MediaAsset>>> + Send;
}
