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
    let read = queries.details.observe(move || (family, id.clone()));
    let data = Signal::derive(move || read.data.get().map(|(value, _)| value));
    let revision = Signal::derive(move || read.data.get().and_then(|(_, revision)| revision));
    let message = Message::new(String::new());
    let can_edit = use_context::<ResourceAccess>().is_some_and(|access| access.0);
    let deletion = ResourceDeletion::new();
    let token = StoredValue::new(token);
    let navigate = leptos_router::hooks::use_navigate();
    let deleted = Callback::new(move |id| {
        queries.deleted(family, id);
        navigate(&format!("/{}", family.route()), Default::default());
    });
    let content = view! {
        <ManagementPage>
            <BackLink href=format!("/{}", family.route())>{move || family.back_label()}</BackLink>
            {message.view()}
            <ResourceDeleteDialog
                deletion=deletion
                api=api
                token=token.get_value()
                on_deleted=deleted
            />
            <QueryStatus
                fetching=read.fetching
                has_data=Signal::derive(move || data.with(Option::is_some))
                error=read.error
                noun=trs!(ui.resource)
                refresh=read.refresh
            />
            {move || {
                data.get()
                    .map(|value| {
                        let id = value.metadata.id;
                        let name = value.metadata.name.clone();
                        let export = value.clone();
                        let export_name = name.clone();
                        let delete_name = name.clone();
                        view! {
                            <PageHeader
                                title=name
                                subtitle=leptos_i18n::t_string!(
                                    i18n, messages.last_updated_arg0, arg0 = (crate::time_display::instant(&serde_json::json!(value.metadata.modified_date))).to_string()
                                )
                            >
                                <ActionButton
                                    disabled=read.fetching
                                    on_click=move |_| read.refresh.run(())
                                >
                                    {tr!(ui.refresh)}
                                </ActionButton>
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
                            <Card class="resource-read-panel">{body(value)}</Card>
                        }
                    })
            }}
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

fn field(label: Signal<String>, value: String) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    view! {
        <div>
            <dt>{move || label.get()}</dt>
            <dd>
                {move || {
                    if value.is_empty() {
                        leptos_i18n::t_string!(i18n, ui.not_set).to_owned()
                    } else {
                        value.clone()
                    }
                }}
            </dd>
        </div>
    }
}
fn content(html: String, plain: String) -> impl IntoView + use<> {
    view! {
        <section class="resource-section">
            <h2>{tr!(ui.content)}</h2>
            {(!html.is_empty())
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
            {(!plain.is_empty())
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
                <section>
                    <h2>
                        {leptos_i18n::t!(
                            i18n, messages.recipients_arg0, arg0 = (rows.with_value(Vec::len)).to_string()
                        )}
                    </h2>
                    <DataTable
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
                                            <td>{row.first_name}</td>
                                            <td>{row.last_name}</td>
                                            <td>{row.position}</td>
                                        </tr>
                                    }
                                })
                                .collect_view()
                        }}
                    </DataTable>
                </section>
            }
            .into_any()
        }
        Content::Template(template) => {
            let attachments = template.attachments;
            view! {
                <dl class="resource-facts">
                    {field(trs!(ui.subject), template.subject)}
                    {field(
                        trs!(ui.visible_from),
                        if template.envelope_sender.is_empty() {
                            leptos_i18n::t_string!(i18n, ui.sending_profile_address).into()
                        } else {
                            template.envelope_sender
                        },
                    )}
                </dl>
                {content(template.html, template.text)}
                <section class="resource-section">
                    <h2>
                        {leptos_i18n::t!(
                            i18n, messages.attachments_arg0, arg0 = (attachments.len()).to_string()
                        )}
                    </h2>
                    <ul>
                        {attachments
                            .into_iter()
                            .map(|a| view! { <li>{a.name}" · "{a.mime}</li> })
                            .collect_view()}
                    </ul>
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
                <dl class="resource-facts">
                    {field(trs!(ui.redirect_after_submission), page.redirect_url)}
                </dl>
            </section>
            {content(page.html, String::new())}
        }
        .into_any(),
        Content::Profile(profile) => {
            let headers = profile.headers;
            view! {
                <dl class="resource-facts">
                    {field(trs!(ui.smtp_host), profile.host)}
                    {field(trs!(ui.sender_address), profile.from_address)}
                    {field(trs!(ui.username), profile.username.unwrap_or_default())}
                    {field(
                        trs!(ui.password),
                        if profile.password.as_deref().is_none_or(str::is_empty) {
                            leptos_i18n::t_string!(i18n, ui.not_configured).into()
                        } else {
                            leptos_i18n::t_string!(i18n, ui.configured).into()
                        },
                    )}
                    {field(trs!(ui.ignore_certificate_errors), yes_no(profile.ignore_cert_errors))}
                </dl>
                <section class="resource-section">
                    <h2>{tr!(ui.additional_headers)}</h2>
                    <dl class="resource-facts">
                        {headers
                            .into_iter()
                            .map(|row| {
                                view! {
                                    <div>
                                        <dt>{row.key}</dt>
                                        <dd>{row.value}</dd>
                                    </div>
                                }
                            })
                            .collect_view()}
                    </dl>
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
