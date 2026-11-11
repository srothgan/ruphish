//! Read-only resource pages, distinct from the create/edit draft lifecycle.
use super::{Api, Family, ResourceAccess, download};
use crate::feedback::Message;
use crate::shared::components::*;
use leptos::prelude::*;
use ruphish_contracts::resource_details::{Content, Resource};
use ruphish_domain::reporting::SortDirection;
use ruphish_ui::preview_document;
use ruphish_ui::{tr, trs};

pub(super) fn show(api: Api, token: String, family: Family, id: String) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let queries = expect_context::<crate::resource_queries::ResourceQueries>();
    let profile_id = id.parse::<i64>().ok();
    let read = queries.details.observe(move || (family, id.clone()));
    let data = Signal::derive(move || read.data.get().map(|(value, _)| value));
    let revision = Signal::derive(move || read.data.get().and_then(|(_, revision)| revision));
    let message = Message::new(String::new());
    let can_edit = use_context::<ResourceAccess>().is_some_and(|access| access.0);
    let deletion = ResourceDeletion::new();
    let token = StoredValue::new(token);
    let testing = (family == Family::Profiles)
        .then_some(profile_id)
        .flatten()
        .map(|id| {
            crate::profile_tests::Testing::new(
                api,
                token.get_value(),
                id,
                Signal::derive(move || {
                    data.with(|d| d.as_ref().map_or(0, |r| r.metadata.revision))
                }),
                can_edit,
                Signal::derive(move || {
                    read.fetching.get() || deletion.busy.get() || data.with(Option::is_none)
                }),
            )
        });
    let navigate = leptos_router::hooks::use_navigate();
    let deleted = Callback::new(move |id| {
        queries.deleted(family, id);
        navigate(&format!("/{}", family.route()), Default::default());
    });
    let content = view! {
        <ManagementPage>
            <BackLink href=format!("/{}", family.route())>{move || family.back_label()}</BackLink>
            <ResourceDeleteDialog
                deletion=deletion
                api=api
                token=token.get_value()
                on_deleted=deleted
            />
            <QueryBoundary
                fetching=read.fetching
                has_data=Signal::derive(move || data.with(Option::is_some))
                error=read.error
                noun=trs!(ui.resource)
                refresh=read.refresh
            >
                {move || {
                    data.get()
                        .map(|value| {
                            let attribution = value.metadata.attribution.clone();
                            let id = value.metadata.id;
                            let name = value.metadata.name.clone();
                            let export = value.clone();
                            let export_name = name.clone();
                            let delete_name = name.clone();
                            view! {
                                <PageHeader title=name>

                                    {testing.map(|testing| testing.actions())}
                                    <RefreshButton
                                        fetching=read.fetching
                                        on_refresh=Callback::new(move |()| {
                                            read.refresh.run(());
                                            if let Some(testing) = testing {
                                                testing.refresh();
                                            }
                                        })
                                    />
                                    <ResourceActions
                                        edit_href=format!("/{}/{id}/edit", family.route())
                                        can_edit=can_edit
                                        disabled=deletion.busy
                                        on_delete=Callback::new(move |()| {
                                            deletion
                                                .select(DeleteTarget {
                                                    id,
                                                    name: delete_name.clone(),
                                                    path: format!("/api/{}/{id}", family.route()),
                                                    revision: revision.get_untracked(),
                                                    description: leptos_i18n::t_string!(
                                                        i18n, ui.this_permanently_removes_the_saved_resource_resources_used_by_unfinished_campaig
                                                    ),
                                                })
                                        })
                                    >
                                        <ActionLink
                                            hidden=!can_edit
                                            disabled=deletion.busy
                                            href=format!("/{}/new?duplicate={id}", family.route())
                                        >
                                            {tr!(ui.duplicate)}
                                        </ActionLink>
                                        <ActionButton on_click=move |_| download(
                                            &export,
                                            &export_name,
                                            message,
                                        )>{tr!(ui.export_json)}</ActionButton>
                                    </ResourceActions>
                                </PageHeader>
                                <div class="settings-stack">
                                    {testing.map(|testing| testing.email_form())}
                                    <DetailCard>
                                        {message.view()} {body(value)}
                                        {testing.map(|testing| testing.results())}
                                        <AttributionInfo value=Signal::derive(move || {
                                            attribution.clone()
                                        }) />
                                    </DetailCard>
                                </div>
                            }
                        })
                }}
            </QueryBoundary>
        </ManagementPage>
    };
    view! {
        <ReadBoundary
            failure=read.failure
            has_data=Signal::derive(move || read.data.with(Option::is_some))
            fetching=read.fetching
            retry=read.refresh
            back_href=format!("/{}", family.route())
            back_label=Signal::derive(move || family.back_label())
        >
            {content}
        </ReadBoundary>
    }
}

fn content(html: String, plain: String) -> impl IntoView + use<> {
    let empty = html.trim().is_empty() && plain.trim().is_empty();
    view! {
        <section class="resource-section">
            <h2>{tr!(ui.content)}</h2>
            {empty.then(|| view! { <p>{tr!(ui.no_content_available)}</p> })}
            {(!html.trim().is_empty())
                .then(|| {
                    view! {
                        <p>
                            {tr!(
                                ui.saved_html_template_expressions_are_shown_as_authored_scripts_and_external_resou
                            )}
                        </p>
                        <iframe
                            class="saved-content-preview"
                            title=trs!(ui.saved_content_preview)
                            sandbox=""
                            referrerpolicy="no-referrer"
                            srcdoc=preview_document(&html)
                        ></iframe>
                        <details>
                            <summary>{tr!(ui.html_source)}</summary>
                            <pre>{html}</pre>
                        </details>
                    }
                })}
            {(!plain.trim().is_empty())
                .then(|| {
                    view! {
                        <h3>{tr!(ui.plain_text)}</h3>
                        <pre>{plain}</pre>
                    }
                })}
        </section>
    }
}
fn body(value: Resource) -> AnyView {
    let i18n = ruphish_ui::localization::use_i18n();

    match value.content {
        Content::Group(group) => {
            let sorting = TableSorting::new("group-recipients", "email", SortDirection::Asc);
            let rows = StoredValue::new(group.targets);
            view! {
                <DetailSection title=trs!(ui.recipients)>
                    <DetailGrid>
                        {detail_text(trs!(ui.recipients), rows.with_value(Vec::len).to_string())}
                    </DetailGrid>
                    <DataTable
                        empty=rows.with_value(Vec::is_empty)
                        sorting=sorting
                        columns=vec!["email", "first_name", "last_name", "position"]
                        label=trs!(ui.recipients)
                        headers=vec![
                            trs!(ui.email),
                            trs!(ui.first_name),
                            trs!(ui.last_name),
                            trs!(ui.position),
                        ]
                    >
                        <TableEmpty slot:empty_content>
                            <p>{tr!(ui.no_recipients_added)}</p>
                        </TableEmpty>

                        {move || {
                            sorting
                                .current
                                .get()
                                .rows(
                                    rows.get_value(),
                                    |row, key| match key {
                                        "first_name" => row.first_name.clone().into(),
                                        "last_name" => row.last_name.clone().into(),
                                        "position" => row.position.clone().into(),
                                        _ => row.email.clone().into(),
                                    },
                                    |row| row.email.clone().into(),
                                )
                                .into_iter()
                                .map(|row| {
                                    view! {
                                        <tr>
                                            <th scope="row">{row.email}</th>
                                            <td>
                                                <ReadValue value=row.first_name />
                                            </td>
                                            <td>
                                                <ReadValue value=row.last_name />
                                            </td>
                                            <td>
                                                <ReadValue value=row.position />
                                            </td>
                                        </tr>
                                    }
                                })
                                .collect_view()
                        }}
                    </DataTable>
                </DetailSection>
            }
            .into_any()
        }
        Content::Template(template) => {
            let attachments = template.attachments;
            let attachment_count = attachments.len().to_string();
            view! {
                <DetailSection title=trs!(ui.details)>
                    <DetailGrid>
                        {detail_text(trs!(ui.subject), template.subject)}
                        {detail_text(
                            trs!(ui.visible_from),
                            if template.envelope_sender.trim().is_empty() {
                                leptos_i18n::t_string!(i18n, ui.sending_profile_address).into()
                            } else {
                                template.envelope_sender
                            },
                        )}
                    </DetailGrid>
                </DetailSection>
                {content(template.html, template.text)}
                <section class="resource-section">
                    <h2>{tr!(ui.attachments)}</h2>
                    <DetailGrid>{detail_text(trs!(ui.attachments), attachment_count)}</DetailGrid>
                    <DetailGrid>
                        {attachments
                            .into_iter()
                            .map(|a| {
                                view! {
                                    <DetailText label=trs!(ui.name) value=a.name />
                                    <DetailText
                                        label=trs!(ui.attachment_type)
                                        value=a.mime_type
                                        fallback=trs!(ui.unknown)
                                    />
                                }
                            })
                            .collect_view()}
                    </DetailGrid>
                </section>
            }
            .into_any()
        }
        Content::LearningPage(page) => view! {
            <p>{tr!(ui.learning_static_help)}</p>
            {content(page.preview_html.unwrap_or_default(), String::new())}
        }
        .into_any(),
        Content::Page(page) => view! {
            <section class="resource-section">
                <h2>{tr!(ui.submission_tracking)}</h2>
                <p>{tr!(ui.all_submissions_recorded)}</p>
                <DetailGrid>
                    {detail_text(trs!(ui.redirect_after_submission), page.redirect_url)}
                </DetailGrid>
            </section>
            {content(page.html, String::new())}
        }
        .into_any(),
        Content::Profile(profile) => {
            let headers = profile.headers;
            let no_headers = headers.is_empty();
            view! {
                <DetailSection title=trs!(ui.details)>
                    <DetailGrid>
                        {detail_text(trs!(ui.smtp_host), profile.host)}
                        {detail_text(trs!(ui.sender_address), profile.from_address)}
                        {detail_text(trs!(ui.username), profile.username.unwrap_or_default())}
                        {detail_text(
                            trs!(ui.password),
                            if !profile.has_password {
                                leptos_i18n::t_string!(i18n, ui.not_configured).into()
                            } else {
                                leptos_i18n::t_string!(i18n, ui.configured).into()
                            },
                        )}
                        {detail_text(
                            trs!(ui.ignore_certificate_errors),
                            yes_no(profile.ignore_cert_errors),
                        )}
                    </DetailGrid>
                </DetailSection>
                <section class="resource-section">
                    <h2>{tr!(ui.additional_headers)}</h2>
                    {no_headers.then(|| view! { <p>{tr!(ui.no_additional_headers)}</p> })}
                    <DetailGrid>
                        {headers
                            .into_iter()
                            .map(|row| {
                                view! {
                                    <div>
                                        <dt>{row.key}</dt>
                                        <dd>
                                            <ReadValue value=row.value />
                                        </dd>
                                    </div>
                                }
                            })
                            .collect_view()}
                    </DetailGrid>
                </section>
            }
            .into_any()
        }
    }
}
fn yes_no(value: bool) -> String {
    let i18n = ruphish_ui::localization::use_i18n();

    if value {
        leptos_i18n::t_string!(i18n, ui.yes)
    } else {
        leptos_i18n::t_string!(i18n, ui.no)
    }
    .into()
}
