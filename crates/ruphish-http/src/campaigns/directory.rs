//! Additive combined management list; legacy campaign APIs remain execution-only.
use super::*;
use app::directory::{Cursor, Filter};
#[derive(Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct QueryPage {
    after: String,
    filter: String,
    sort: Option<ruphish_application::pagination::DirectorySort>,
    order: Option<ruphish_application::reporting::SortDirection>,
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
    let after = if let Some(sort) = page.sort {
        Cursor {
            sorted: Some(sorted_page(
                sort,
                page.order.unwrap_or_default(),
                format!(
                    "directory:{}:{}:{}:{}",
                    caller.identity.id,
                    filter.drafts(),
                    filter.executions(),
                    filter.completed()
                ),
                &page.after,
            )?),
            ..Default::default()
        }
    } else {
        if page.order.is_some() {
            return Err(invalid());
        }
        Cursor::parse(&page.after).map_err(error)?
    };
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
            launch_date: row.launch.as_deref().and_then(date),
        })
        .collect();
    response(
        StatusCode::OK,
        &reads::Directory {
            items,
            next_after: page.next_after,
        },
        None,
    )
}

/// Decode a bounded, versioned continuation and reject selection mismatches.
pub(super) fn sorted_page<S: serde::de::DeserializeOwned + PartialEq>(
    sort: S,
    order: ruphish_application::reporting::SortDirection,
    scope: String,
    after: &str,
) -> Result<ruphish_application::pagination::SortedPage<S>, Rejection> {
    let invalid = || error(Error::Invalid("Invalid sorted page cursor".into()));
    let position = if after.is_empty() {
        None
    } else {
        if after.len() > 8192 {
            return Err(invalid());
        }
        let cursor: ruphish_application::pagination::Cursor<S> =
            serde_json::from_str(after).map_err(|_| invalid())?;
        if !cursor.valid(&sort, order, &scope) {
            return Err(invalid());
        }
        Some(cursor.position)
    };
    Ok(ruphish_application::pagination::SortedPage {
        sort,
        order,
        scope,
        after: position,
    })
}
