//! Additive combined management list; legacy campaign APIs remain execution-only.
use super::*;
use app::directory::{Cursor, Filter};
#[derive(Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct QueryPage {
    after: String,
    filter: String,
}
pub(super) async fn list(
    c: Context,
    page: Result<Query<QueryPage>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Rejection> {
    let caller = c.api(None).await?;
    let invalid = || error(Error::Invalid("Invalid campaign directory query".into()));
    let Query(page) = page.map_err(|_| invalid())?;
    let filter = match page.filter.as_str() {
        "" | "all" => Filter::All,
        "drafts" => Filter::Drafts,
        "active" => Filter::Active,
        "completed" => Filter::Completed,
        "executions" => Filter::Executions,
        _ => return Err(invalid()),
    };
    let after = Cursor::parse(&page.after).map_err(error)?;
    let page = c
        .campaigns()
        .directory(caller.identity, filter, after, 50)
        .await
        .map_err(error)?;
    let items = page
        .items
        .into_iter()
        .map(|row| reads::DirectoryRow {
            id: row.id,
            kind: if row.revision.is_some() {
                "draft".into()
            } else {
                "campaign".into()
            },
            revision: row.revision,
            name: row.name,
            status: row.status,
            total: row.total,
            launch_date: row.launch.as_deref().map(date),
        })
        .collect();
    response(
        StatusCode::OK,
        &json!(reads::Directory {
            items,
            next_after: page.next_after
        }),
        None,
    )
}
