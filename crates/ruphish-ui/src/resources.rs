//! Resource lists and full-page editors using the preserved HTTP boundary.
use crate::account::Api;
use crate::feedback::Message;
use crate::shared::components::*;
use leptos::{html, prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use ruphish_ui::{
    import_drafts::{ImportDrafts, ImportedDraft},
    import_state::{ImportError, ImportReview},
    preview_document,
    resource_state::ResourceDraft,
};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub(super) struct ResourceAccess(pub bool);

#[derive(Clone, Copy)]
pub(super) enum Screen {
    List,
    View,
    Edit,
}

#[path = "resource_view.rs"]
mod resource_view;

/// Keep page lifetime tied to its ID and duplicate source; the shell stays mounted.
pub(super) fn detail(api: Api, token: String, family: Family, editing: bool) -> impl IntoView {
    use leptos_router::{
        NavigateOptions,
        hooks::{use_navigate, use_params_map, use_query_map},
    };
    let params = use_params_map();
    let query = use_query_map();
    let navigate = use_navigate();
    let drafts = use_context::<ImportDrafts>();
    let replace_url = Callback::new(move |url: String| {
        navigate(
            &url,
            NavigateOptions {
                replace: true,
                ..Default::default()
            },
        )
    });
    view! {<For each=move||vec![(params.with(|p|p.get("id")).unwrap_or_default(), query.with(|q|q.get("duplicate")), query.with(|q|q.get("draft")))] key=|key|key.clone() children=move|(id,duplicate,draft_id)|{
        if id == "new" && let Some(draft_id) = draft_id {
            if let Some(imported) = drafts.and_then(|store| store.get(family.route(), &draft_id)) {
                on_cleanup(move || { if let Some(store) = drafts { store.remove(&draft_id); } });
                edit(api,token.clone(),family,id,None,replace_url,Some(imported)).into_any()
            } else {
                view!{<ManagementPage><h1>"This draft is no longer available"</h1><p class="form-message error" role="alert">{ImportError::DraftUnavailable.to_string()}</p><p>"Imported drafts stay in this tab until saved or discarded. Refreshing, closing the tab or changing accounts clears them."</p><ActionLink style=ButtonStyle::Primary href=format!("/{}?import=1",family.route())>"Import again"</ActionLink></ManagementPage>}.into_any()
            }
        } else if editing || id == "new" { edit(api,token.clone(),family,id,duplicate,replace_url,None).into_any() }
        else { resource_view::show(api,token.clone(),family,id).into_any() }
    }/>}
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Family {
    Groups,
    Templates,
    Pages,
    Profiles,
}
impl Family {
    fn route(self) -> &'static str {
        match self {
            Self::Groups => "groups",
            Self::Templates => "templates",
            Self::Pages => "pages",
            Self::Profiles => "smtp",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Groups => "Recipient groups",
            Self::Templates => "Email templates",
            Self::Pages => "Landing pages",
            Self::Profiles => "Sending profiles",
        }
    }
    fn singular(self) -> &'static str {
        match self {
            Self::Groups => "group",
            Self::Templates => "email template",
            Self::Pages => "landing page",
            Self::Profiles => "sending profile",
        }
    }
    fn plural(self) -> &'static str {
        match self {
            Self::Groups => "recipient groups",
            Self::Templates => "email templates",
            Self::Pages => "landing pages",
            Self::Profiles => "sending profiles",
        }
    }
    fn empty_title(self) -> &'static str {
        match self {
            Self::Groups => "No recipient groups yet",
            Self::Templates => "No email templates yet",
            Self::Pages => "No landing pages yet",
            Self::Profiles => "No sending profiles yet",
        }
    }
    fn empty_guidance(self) -> &'static str {
        match self {
            Self::Groups => "Create a group or import recipients to use as a campaign audience.",
            Self::Templates => "Create or import an email template to send in a campaign.",
            Self::Pages => "Create or import a landing page for recipients who follow a link.",
            Self::Profiles => "Add a sending profile and verify it before launching a campaign.",
        }
    }
    fn empty(self) -> Value {
        match self {
            Self::Groups => json!({"id":0,"name":"","targets":[]}),
            Self::Templates => {
                json!({"id":0,"name":"","subject":"","envelope_sender":"","html":"","text":"","attachments":[]})
            }
            Self::Pages => {
                json!({"id":0,"name":"","html":"","capture_credentials":false,"capture_passwords":false,"redirect_url":""})
            }
            Self::Profiles => {
                json!({"id":0,"name":"","host":"","from_address":"","username":"","password":"","ignore_cert_errors":false,"interface_type":"SMTP","headers":[]})
            }
        }
    }
}
fn download(value: &Value, name: &str, message: Message) {
    match serde_json::to_vec_pretty(value) {
        Ok(bytes) => {
            if let Err(e) = crate::editor::download_template(&bytes, name) {
                message.error(format!("Download failed: {e:?}"));
            }
        }
        Err(e) => message.error(e.to_string()),
    }
}
pub(super) fn list(api: Api, token: String, family: Family) -> impl IntoView {
    let can_edit = use_context::<ResourceAccess>().is_some_and(|access| access.0);
    let token = StoredValue::new(token);
    let query = leptos_router::hooks::use_query_map();
    let import_open = RwSignal::new(
        can_edit && query.with_untracked(|q| q.get("import").as_deref() == Some("1")),
    );
    let rows = RwSignal::new(Vec::<Value>::new());
    let message = Message::new(String::new());
    let loading = RwSignal::new(true);
    let failed = RwSignal::new(false);
    let search = RwSignal::new(String::new());
    let refresh = RwSignal::new(0u64);
    let deletion = ResourceDeletion::new();
    let generation = StoredValue::new(0u64);
    Effect::new(move |_| {
        refresh.get();
        generation.update_value(|n| *n += 1);
        let ticket = generation.get_value();
        loading.set(true);
        failed.set(false);
        spawn_local(async move {
            let result = api.get(&format!("/api/{}/summary", family.route())).await;
            if generation.get_value() != ticket {
                return;
            }
            match result {
                Ok(v) => rows.set(
                    v[if family == Family::Groups {
                        "groups"
                    } else {
                        "items"
                    }]
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
                ),
                Err(e) => {
                    failed.set(true);
                    message.error(e);
                }
            }
            loading.set(false);
        });
    });
    let deleted = Callback::new(move |id| {
        generation.update_value(|n| *n += 1);
        loading.set(false);
        failed.set(false);
        rows.update(|rows| rows.retain(|row| row["id"].as_i64() != Some(id)));
        message.status("Resource deleted.".into());
    });
    let visible = Signal::derive(move || {
        let needle = search.get().to_lowercase();
        rows.get()
            .into_iter()
            .filter(|v| {
                v["name"]
                    .as_str()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&needle)
            })
            .collect::<Vec<_>>()
    });
    view! {<ManagementPage>
    <PageHeader title=family.title().to_owned() subtitle=match family{Family::Groups=>"Organize recipients and review imported data before saving.",Family::Templates=>"Compose messages, manage attachments and send a test.",Family::Pages=>"Prepare landing-page content and optional interaction defaults.",Family::Profiles=>"Configure mail connections and verify them before use."}.to_owned()>
        <ActionButton hidden=!can_edit on_click=move |_|import_open.set(true)>"Import"</ActionButton><ActionLink hidden=!can_edit style=ButtonStyle::Primary href=format!("/{}/new",family.route())>{format!("Create {}",family.singular())}</ActionLink>
    </PageHeader>
    {move||(can_edit && import_open.get()).then(||import_panel(api,token.get_value(),family,import_open))}
    {message.view()}
    <ResourceDeleteDialog deletion=deletion api=api token=token.get_value() on_deleted=deleted/>
    <ListToolbar search=search count=Signal::derive(move || visible.with(Vec::len)) noun=family.plural()/>
    <Show when=move||loading.get()><p role="status">{format!("Loading {}…", family.plural())}</p></Show>
    <Show when=move||!loading.get()&&failed.get()><ActionButton on_click=move |_|refresh.update(|v|*v+=1)>{format!("Retry loading {}", family.plural())}</ActionButton></Show>
    <DataTable label=family.title() empty=Signal::derive(move || visible.with(Vec::is_empty)) headers=vec!["Name",if family==Family::Groups{"Recipients"}else{"Active campaigns"},"Last modified","Actions"]>
    {move||visible.get().into_iter().map(|v|{let id=v["id"].as_i64().unwrap_or(0);let name=v["name"].as_str().unwrap_or("Unnamed").to_owned();let delete_value=v.clone();view!{<tr><th scope="row"><a href=format!("/{}/{id}",family.route())>{name}</a></th><td>{v[if family==Family::Groups{"num_targets"}else{"active_campaigns"}].as_i64().unwrap_or(0)}</td><td>{crate::time_display::instant(&v["modified_date"])}</td><td><ResourceActions view_href=format!("/{}/{id}",family.route()) edit_href=format!("/{}/{id}/edit",family.route()) can_edit=can_edit size=ControlSize::Small disabled=deletion.busy on_delete=Callback::new(move |()| deletion.select(DeleteTarget { id, name:delete_value["name"].as_str().unwrap_or("resource").to_owned(), path:format!("/api/{}/{id}",family.route()), revision:None, description:"This permanently removes the saved resource. Resources used by unfinished campaigns cannot be deleted." }))><ActionLink hidden=!can_edit size=ControlSize::Small disabled=deletion.busy href=format!("/{}/new?duplicate={id}",family.route())>"Duplicate"</ActionLink></ResourceActions></td></tr>}}).collect_view()}
    </DataTable>
    <ListEmptyState ready=Signal::derive(move || !loading.get() && !failed.get()) count=Signal::derive(move || visible.with(Vec::len)) search=search title=family.empty_title() guidance=family.empty_guidance()/>
    </ManagementPage>}
}

#[derive(Clone, Copy)]
struct Editor {
    draft: RwSignal<ResourceDraft>,
    message: Message,
    epoch: RwSignal<u64>,
    busy: RwSignal<bool>,
    visual_pending: RwSignal<bool>,
    readonly: bool,
    replace_url: Callback<String>,
    import_review: Option<RwSignal<ImportReview>>,
}
impl Editor {
    fn text(self, key: &str) -> String {
        self.draft
            .with(|d| d.value()[key].as_str().unwrap_or("").to_owned())
    }
    fn set(self, key: &str, value: Value) {
        self.draft.update(|d| d.edit(|v| v[key] = value));
    }
    fn field(
        self,
        key: &'static str,
        label: &'static str,
        input_type: &'static str,
    ) -> impl IntoView {
        view! {<Field label=label><TextInput id=key name=key input_type=input_type autocomplete="off" value=Signal::derive(move||self.text(key)) on_input=move|e|self.set(key,json!(event_target_value(&e)))/></Field>}
    }
    fn flag(self, key: &'static str, label: &'static str) -> impl IntoView {
        view! {<Checkbox label=label checked=Signal::derive(move||self.draft.with(|d|d.value()[key].as_bool().unwrap_or(false))) on_toggle=Callback::new(move|v|self.set(key,json!(v)))/>}
    }
}
fn edit(
    api: Api,
    token: String,
    family: Family,
    id: String,
    duplicate: Option<String>,
    replace_url: Callback<String>,
    imported: Option<ImportedDraft>,
) -> impl IntoView {
    let token = StoredValue::new(token);
    let requested = StoredValue::new(id);
    let loading = RwSignal::new(true);
    let load_failed = RwSignal::new(false);
    let is_imported = imported.is_some();
    let import_review = imported.as_ref().map(|i| RwSignal::from(i.review.clone()));
    let draft = imported
        .map(|i| RwSignal::from(i.draft))
        .unwrap_or_else(|| RwSignal::new(ResourceDraft::new(family.empty(), None)));
    let editor = Editor {
        draft,
        import_review,
        message: Message::new(String::new()),
        epoch: RwSignal::new(0),
        busy: RwSignal::new(false),
        visual_pending: RwSignal::new(false),
        readonly: !use_context::<ResourceAccess>().is_some_and(|access| access.0),
        replace_url,
    };
    let test_open = RwSignal::new(false);
    let reload = RwSignal::new(0u64);
    Effect::new(move |_| {
        reload.get();
        loading.set(true);
        load_failed.set(false);
        editor.message.clear();
        let duplicate = duplicate.clone();
        spawn_local(async move {
            if is_imported {
                loading.set(false);
                return;
            }
            let id = requested.get_value();
            let target = if id == "new" {
                duplicate.clone()
            } else {
                Some(id)
            };
            if let Some(target) = target {
                match api
                    .resource(
                        "GET",
                        &format!("/api/{}/{target}", family.route()),
                        &token.get_value(),
                        None,
                        None,
                    )
                    .await
                {
                    Ok((mut value, revision)) => {
                        if duplicate.is_some() {
                            value["id"] = json!(0);
                            value["name"] = json!(format!(
                                "{} copy",
                                value["name"].as_str().unwrap_or("Resource")
                            ));
                            value.as_object_mut().map(|v| v.remove("modified_date"));
                        }
                        let loaded = if duplicate.is_some() {
                            let mut draft = ResourceDraft::new(family.empty(), None);
                            draft.edit(|draft| *draft = value);
                            draft
                        } else {
                            ResourceDraft::new(value, revision)
                        };
                        editor.draft.set(loaded);
                        editor.epoch.update(|v| *v += 1);
                    }
                    Err(e) => {
                        editor.message.error(e);
                        load_failed.set(true);
                    }
                }
            }
            loading.set(false);
        });
    });
    crate::navigation::protect_draft(move || {
        editor.draft.with(ResourceDraft::dirty) || editor.visual_pending.get() || editor.busy.get()
    });
    let save = move |_| {
        spawn_local(async move {
            if save_draft(api, token.get_value(), family, editor).await {
                finish_edit(editor, family);
            }
        });
    };
    let content = match family {
        Family::Groups => group_fields(editor).into_any(),
        Family::Templates | Family::Pages => content_fields(api, token, editor, family).into_any(),
        Family::Profiles => profile_fields(editor).into_any(),
    };
    view! {<ManagementPage><BackLink href=format!("/{}",family.route())>{format!("Back to {}",family.title().to_lowercase())}</BackLink>
    <PageHeader title=Signal::derive(move||{let name=editor.text("name");if name.is_empty(){format!("New {}",family.singular())}else{name}})
        status=true subtitle=Signal::derive(move||if editor.busy.get(){"Saving or checking…"}else if loading.get(){"Loading resource…"}else if load_failed.get(){"Resource could not be loaded"}else if editor.readonly{"Read-only access"}else if editor.visual_pending.get(){"Apply or cancel visual edits before saving"}else if editor.draft.with(ResourceDraft::dirty){"Unsaved changes"}else if editor.draft.with(|d|d.value()["id"].as_i64().unwrap_or(0)==0){"Not saved yet"}else{"No unsaved changes"}.to_owned())>
    {(family==Family::Templates||family==Family::Profiles).then(||view!{<ActionButton disabled=Signal::derive(move||loading.get()||load_failed.get()||editor.readonly||editor.busy.get()||editor.visual_pending.get()) on_click=move |_|test_open.set(true)>"Send test"</ActionButton>})}

    </PageHeader>
    {matches!(family,Family::Templates|Family::Pages|Family::Profiles).then(||view!{<p class="resource-usage">"Campaigns use the current saved version of this resource. Editing it can affect unfinished campaigns."</p>})}
    {editor.import_review.map(|review|import_review_panel(review,editor.message))}
    {editor.message.view()}<Show when=move||!loading.get()&&load_failed.get()><ActionButton on_click=move |_|reload.update(|n|*n+=1)>"Retry loading resource"</ActionButton></Show><Show when=move||loading.get()><p>"Loading resource…"</p></Show>
    <FormFields disabled=Signal::derive(move||load_failed.get()||editor.readonly||loading.get()||editor.busy.get())>{editor.field("name","Name","text")}{content}</FormFields>
    <FormFooter spread=true><ActionButton style=ButtonStyle::Ghost disabled=Signal::derive(move||loading.get()||load_failed.get()||editor.busy.get()) on_click=move |_|{editor.draft.update(ResourceDraft::cancel);editor.epoch.update(|v|*v+=1);let id=editor.draft.with_untracked(|d|d.value()["id"].as_i64().unwrap_or(0)); editor.replace_url.run(if id>0{format!("/{}/{id}",family.route())}else{format!("/{}",family.route())});}>"Cancel changes"</ActionButton><SaveButton disabled=Signal::derive(move||loading.get()||load_failed.get()||editor.readonly||editor.busy.get()||editor.visual_pending.get()||editor.import_review.is_some_and(|r|!r.with(ImportReview::ready))) on_click=Callback::new(save) label=Signal::derive(move||if editor.draft.with(|d|d.value()["id"].as_i64().unwrap_or(0)==0){format!("Create {}",family.singular())}else{"Save changes".into()})/></FormFooter>
    {move||test_open.get().then(||test_panel(api,token.get_value(),family,editor,test_open))}
    </ManagementPage>}
}
fn finish_edit(editor: Editor, family: Family) {
    if let Some(id) = editor.draft.with_untracked(ResourceDraft::completed_id) {
        editor.replace_url.run(format!("/{}/{id}", family.route()));
    }
}

async fn save_draft(api: Api, token: String, family: Family, editor: Editor) -> bool {
    if editor.readonly
        || editor.busy.get_untracked()
        || editor.visual_pending.get_untracked()
        || editor
            .import_review
            .is_some_and(|r| !r.with_untracked(ImportReview::ready))
    {
        return false;
    }
    editor.busy.set(true);
    editor.message.clear();
    let current = editor.draft.get_untracked();
    let submitted = current.value().clone();
    let id = submitted["id"].as_i64().unwrap_or(0);
    let path = if id == 0 {
        format!("/api/{}/", family.route())
    } else {
        format!("/api/{}/{id}", family.route())
    };
    let result = api
        .resource(
            if id == 0 { "POST" } else { "PUT" },
            &path,
            &token,
            Some(&submitted),
            current.revision(),
        )
        .await;
    editor.busy.set(false);
    match result {
        Ok((value, revision)) => {
            editor
                .draft
                .update(|d| d.saved(&submitted, value, revision));
            editor.epoch.update(|v| *v += 1);
            editor.message.status(
                if editor.draft.with_untracked(ResourceDraft::dirty) {
                    "Saved the submitted version. Newer changes remain unsaved."
                } else {
                    "Changes saved."
                }
                .into(),
            );
            true
        }
        Err(e) => {
            editor.message.error(e);
            false
        }
    }
}
fn group_fields(editor: Editor) -> impl IntoView {
    view! {<section class="resource-section"><div class="management-heading"><h2>"Recipients"</h2><ActionButton kind="button" on_click=move |_|editor.draft.update(|d|d.edit(|v|{if let Some(rows)=v["targets"].as_array_mut(){rows.push(json!({"email":"","first_name":"","last_name":"","position":""}));}}))>"Add recipient"</ActionButton></div><DataTable label="Recipients" headers=vec!["Email","First name","Last name","Position","Action"] class="recipient-table">
    <For each=move||0..editor.draft.with(|d|d.value()["targets"].as_array().map_or(0,Vec::len)) key=|i|*i children=move|index|view!{<tr>{["email","first_name","last_name","position"].into_iter().map(move|key|view!{<td><TextInput aria_label=format!("Recipient {} {key}",index+1) value=Signal::derive(move||editor.draft.with(|d|d.value()["targets"][index][key].as_str().unwrap_or("").to_owned())) on_input=move|e|editor.draft.update(|d|d.edit(|v|v["targets"][index][key]=json!(event_target_value(&e))))/></td>}).collect_view()}<td><ActionButton kind="button" on_click=move |_|editor.draft.update(|d|d.edit(|v|{if let Some(rows)=v["targets"].as_array_mut() && index<rows.len(){rows.remove(index);}}))>"Remove"</ActionButton></td></tr>}/>
    </DataTable></section>}
}
fn content_workspace(
    api: Api,
    token: StoredValue<String>,
    editor: Editor,
    family: Family,
) -> impl IntoView {
    use ruphish_ui::preview_state::ContentPreview;
    let selected_tab = RwSignal::new(0usize);
    let showing_preview = Signal::derive(move || selected_tab.get() == 1);
    let preview = RwSignal::new(ContentPreview::default());
    let html_source = Memo::new(move |_| editor.text("html"));
    let retry = RwSignal::new(0u64);
    Effect::new(move |_| {
        let visible = showing_preview.get();
        let source = html_source.get();
        retry.get();
        if !visible {
            preview.update(ContentPreview::clear);
            return;
        }
        let Some(generation) = preview.try_update(|p| p.begin(source.clone())) else {
            return;
        };
        spawn_local(async move {
            let body = json!({"html":source,"recipient":{"email":"alice@example.test","first_name":"Alice","last_name":"Example","position":"Analyst"}});
            let result = api
                .resource(
                    "POST",
                    "/api/util/preview",
                    &token.get_value(),
                    Some(&body),
                    None,
                )
                .await
                .and_then(|(value, _)| {
                    value["html"]
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "The preview response did not contain HTML.".into())
                });
            if showing_preview.get_untracked() && html_source.get_untracked() == source {
                preview.update(|p| p.finish(generation, &source, result));
            }
        });
    });
    view! {<section class="content-workspace">
        <div class="content-heading"><h2>{if family==Family::Templates {"Email HTML"} else {"Landing-page HTML"}}</h2>
        <Tabs label="HTML editing and preview" selected=selected_tab tabs=vec![
            Tab { id: "content-edit", label: "Edit", disabled: Signal::derive(||false) },
            Tab { id: "content-preview", label: "Preview", disabled: editor.visual_pending.into() },
        ]/></div>
        <p class="content-tab-help" hidden=move||!editor.visual_pending.get()>"Apply or cancel visual edits before previewing."</p>
        <TabPanel id="content-edit" selected=Signal::derive(move||!showing_preview.get())>
            {crate::editor::content_editor(Signal::derive(move||editor.text("html")),Callback::new(move|value|editor.set("html",json!(value))),editor.epoch,editor.message,editor.visual_pending,editor.readonly)}
        </TabPanel>
        <TabPanel id="content-preview" class="content-preview-panel" selected=showing_preview busy=Signal::derive(move||preview.with(|p|p.loading))>
            <p class="content-tab-help">"Current draft with sample recipient Alice Example. Scripts, external resources and submissions are blocked."{(family==Family::Templates).then_some(" Email clients may render it differently.")}</p>
            <Show when=move||preview.with(|p|p.loading)><p role="status">"Rendering preview…"</p></Show>
            {move||preview.get().error.map(|error|view!{<p class="form-message error" role="alert">{error}</p><ActionButton kind="button" on_click=move |_|retry.update(|v|*v+=1)>"Retry preview"</ActionButton>})}
            {move||preview.get().html.map(|html|view!{<iframe title="Current HTML draft preview" sandbox="" referrerpolicy="no-referrer" srcdoc=preview_document(&html)></iframe>})}
        </TabPanel>
    </section>}
}
fn content_fields(
    api: Api,
    token: StoredValue<String>,
    editor: Editor,
    family: Family,
) -> impl IntoView {
    let files = NodeRef::<html::Input>::new();
    let add = move |event| {
        let input = event_target::<web_sys::HtmlInputElement>(&event);
        let Some(files) = input.files() else {
            return;
        };
        let epoch = editor.epoch.get_untracked();
        spawn_local(async move {
            for index in 0..files.length() {
                if let Some(file) = files.get(index) {
                    if file.size() > 16.0 * 1024.0 * 1024.0 {
                        editor
                            .message
                            .error("Each attachment must be at most 16 MiB.".into());
                        continue;
                    }
                    match wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await {
                        Ok(buffer) => {
                            if editor.epoch.get_untracked() != epoch {
                                return;
                            }
                            let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                            let mime = if file.type_().is_empty() {
                                "application/octet-stream".into()
                            } else {
                                file.type_()
                            };
                            editor.draft.update(|d| d.attach(file.name(), mime, &bytes));
                        }
                        Err(_) => editor.message.error("Could not read attachment.".into()),
                    }
                }
            }
            input.set_value("");
        });
    };
    view! {<section class="resource-section">
    {(family==Family::Templates).then(||view!{<div class="resource-columns">{editor.field("subject","Subject","text")}{editor.field("envelope_sender","Visible From (blank uses sending profile)","text")}</div>})}
    {content_workspace(api,token,editor,family)}
    {(family==Family::Templates).then(||view!{<label class="text-alternative">"Plain-text alternative"<Textarea rows=6 value=Signal::derive(move||editor.text("text")) on_input=move|e|editor.set("text",json!(event_target_value(&e)))/></label><div class="resource-section"><h2>"Attachments"</h2><input node_ref=files hidden type="file" multiple on:change=add/><ActionButton kind="button" on_click=move |_|{if let Some(node)=files.get(){node.click();}}>"Add attachments"</ActionButton><ul>{move||editor.draft.with(|d|d.value()["attachments"].as_array().cloned().unwrap_or_default()).into_iter().enumerate().map(|(i,a)|view!{<li>{a["name"].as_str().unwrap_or("").to_owned()}<ActionButton kind="button" on_click=move |_|editor.draft.update(|d|{d.remove_attachment(i);})>"Remove"</ActionButton></li>}).collect_view()}</ul></div>})}
    {(family==Family::Pages).then(||view!{<section class="resource-section"><h2>"Optional campaign defaults"</h2><p>"Public pages preserve field names and styling, but block authored scripts, embedded frames and external resources. Use embedded PNG, JPEG or GIF images. Preview the final page before launch; form submissions send only event metadata and optional password presence."</p><p>"Suggested submission settings for campaigns using this page. Leave tracking off and the redirect blank if no defaults are needed."</p>{editor.flag("capture_credentials","Record form submission")}{editor.flag("capture_passwords","Record password presence")}{editor.field("redirect_url","Redirect URL after submission (optional)","text")}<p>"Track only whether fields were filled and submitted. Entered usernames, passwords and other field values are never collected."</p></section>})}
    </section>}
}
fn profile_fields(editor: Editor) -> impl IntoView {
    view! {<section class="resource-section"><h2>"Connection"</h2><div class="resource-columns">{editor.field("host","SMTP host and port","text")}{editor.field("from_address","Sender address","text")}{editor.field("username","Username (optional)","text")}{editor.field("password","Password","password")}</div><p>"Port defaults to 25. Port 465 uses implicit TLS; STARTTLS is used when advertised. Passwords are masked, never replaced with a placeholder."</p>{editor.flag("ignore_cert_errors","Ignore SMTP certificate errors")}</section><section class="resource-section"><div class="management-heading"><h2>"Additional headers"</h2><ActionButton kind="button" on_click=move |_|editor.draft.update(|d|d.edit(|v|{if let Some(rows)=v["headers"].as_array_mut(){rows.push(json!({"key":"","value":""}));}}))>"Add header"</ActionButton></div><For each=move||0..editor.draft.with(|d|d.value()["headers"].as_array().map_or(0,Vec::len)) key=|i|*i children=move|index|view!{<div class="header-row">{["key","value"].into_iter().map(move|key|view!{<label>{if key=="key"{"Header name"}else{"Value"}}<TextInput value=Signal::derive(move||editor.draft.with(|d|d.value()["headers"][index][key].as_str().unwrap_or("").to_owned())) on_input=move|e|editor.draft.update(|d|d.edit(|v|v["headers"][index][key]=json!(event_target_value(&e))))/></label>}).collect_view()}<ActionButton kind="button" on_click=move |_|editor.draft.update(|d|d.edit(|v|{if let Some(rows)=v["headers"].as_array_mut() && index<rows.len(){rows.remove(index);}}))>"Remove"</ActionButton></div>}/></section>}
}

fn import_panel(api: Api, token: String, family: Family, open: RwSignal<bool>) -> impl IntoView {
    let token = StoredValue::new(token);
    let content = RwSignal::new(String::new());
    let mode = RwSignal::new("json".to_owned());
    let busy = RwSignal::new(false);
    let message = Message::new(String::new());
    let file = NodeRef::<html::Input>::new();
    let convert = RwSignal::new(false);
    let columns = RwSignal::new([
        "Email".to_owned(),
        "First Name".to_owned(),
        "Last Name".to_owned(),
        "Position".to_owned(),
    ]);
    let drafts = use_context::<ImportDrafts>();
    let navigate = leptos_router::hooks::use_navigate();
    // Scoped async work is canceled when the dialog or authenticated shell disappears.
    crate::navigation::protect_draft(move || !content.get().is_empty() || busy.get());
    let read = move |event| {
        let input = event_target::<web_sys::HtmlInputElement>(&event);
        let Some(file) = input.files().and_then(|f| f.get(0)) else {
            return;
        };
        input.set_value("");
        message.clear();
        if file.size() > 8.0 * 1024.0 * 1024.0 {
            message.error(ImportError::TooLarge.to_string());
            return;
        }
        let extension = file
            .name()
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let kind = match extension.as_str() {
            "json" => "json",
            "csv" if family == Family::Groups => "csv",
            "eml" if family == Family::Templates => "email",
            "html" | "htm" if matches!(family, Family::Templates | Family::Pages) => "html",
            _ => {
                message.error(ImportError::UnsupportedFormat.to_string());
                return;
            }
        };
        busy.set(true);
        spawn_local(async move {
            match wasm_bindgen_futures::JsFuture::from(file.text()).await {
                Ok(value) => {
                    content.set(value.as_string().unwrap_or_default());
                    mode.set(kind.into());
                }
                Err(_) => message.error(ImportError::ReadFailed.to_string()),
            }
            busy.set(false);
        });
    };
    let review = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        message.clear();
        let source = content.get_untracked();
        let kind = mode.get_untracked();
        let columns = columns.get_untracked();
        let convert = convert.get_untracked();
        let navigate = navigate.clone();
        spawn_local(async move {
            let result = if source.len() > 8 * 1024 * 1024 {
                Err(ImportError::TooLarge)
            } else if kind == "json" {
                ruphish_ui::import_state::parse_json(&source)
            } else if kind == "html" {
                Ok(json!({"html":source}))
            } else {
                let (path, payload) = match kind.as_str() {
                    "csv" => (
                        "/api/import/group/review",
                        json!({"content":source,"columns":columns}),
                    ),
                    "email" => (
                        "/api/import/email",
                        json!({"content":source,"convert_links":convert}),
                    ),
                    _ => ("/api/import/site", json!({"url":source})),
                };
                api.resource("POST", path, &token.get_value(), Some(&payload), None)
                    .await
                    .map(|(v, _)| v)
                    .map_err(ImportError::RequestFailed)
            }
            .and_then(|value| {
                ruphish_ui::import_state::prepare_creation(family.route(), &kind, value)
            });
            match result {
                Ok(imported) => {
                    let mut draft = ResourceDraft::new(family.empty(), None);
                    draft.merge_import(&imported.fields);
                    let result = drafts
                        .ok_or(ImportError::DraftUnavailable)
                        .and_then(|store| store.insert(family.route(), draft, imported.review));
                    match result {
                        Ok(id) => {
                            content.set(String::new());
                            busy.set(false);
                            open.set(false);
                            navigate(
                                &format!("/{}/new?draft={id}", family.route()),
                                Default::default(),
                            );
                            return;
                        }
                        Err(error) => message.error(error.to_string()),
                    }
                }
                Err(error) => message.error(error.to_string()),
            }
            busy.set(false);
        });
    };
    view! {<Dialog title=format!("Import {}",family.singular()) on_cancel=Callback::new(move |_|open.set(false))>
        <div class="management-heading"><h2 id="import-heading">{format!("Import {}",family.singular())}</h2></div>
        <p>"Load content into a new Create form to review and finish. Nothing is saved yet."</p>
        <fieldset disabled=move||busy.get() class="import-fields">
        <input hidden node_ref=file type="file" accept=match family{Family::Groups=>".json,.csv",Family::Templates=>".json,.eml,.html,.htm",Family::Pages=>".json,.html,.htm",Family::Profiles=>".json"} on:change=read/>
        <ActionButton on_click=move |_|{if let Some(file)=file.get(){file.click();}}>"Choose file"</ActionButton>
        <Select label="Import format" value=mode on_change=Callback::new(move|v:String|{mode.set(v);content.set(String::new());message.clear();})>
        <option value="json">"Resource JSON"</option>
        {(family==Family::Groups).then(||view!{<option value="csv">"Recipients CSV"</option>})}
        {(family==Family::Templates).then(||view!{<option value="email">"Email (.eml)"</option>})}
        {matches!(family,Family::Templates|Family::Pages).then(||view!{<option value="html">"HTML document"</option>})}
        {(family==Family::Pages).then(||view!{<option value="site">"Website URL"</option>})}
        </Select>
        <label>{move||if mode.get()=="site"{"Website URL"}else{"File content or pasted content"}}<Textarea rows=7 value=Signal::derive(move||content.get()) on_input=move|e|content.set(event_target_value(&e))/></label>
        <Show when=move||mode.get()=="csv"><div class="resource-columns">{["Email column","First-name column","Last-name column","Position column"].into_iter().enumerate().map(|(i,label)|view!{<label>{label}<TextInput value=Signal::derive(move||columns.get()[i].clone()) on_input=move|e|columns.update(|v|v[i]=event_target_value(&e))/></label>}).collect_view()}</div></Show>
        <Show when=move||mode.get()=="email"><Checkbox label="Replace links with the recipient URL" checked=convert on_toggle=Callback::new(move|v|convert.set(v))/></Show>
        </fieldset>{message.view()}
        <div class="import-actions"><ActionButton style=ButtonStyle::Ghost on_click=move |_|open.set(false)>"Cancel import"</ActionButton><ActionButton style=ButtonStyle::Primary disabled=Signal::derive(move||busy.get()||content.get().trim().is_empty()) on_click=review>{move||if busy.get(){"Reading import…"}else{"Continue to create"}}</ActionButton></div>
    </Dialog>}
}

fn import_review_panel(review: RwSignal<ImportReview>, message: Message) -> impl IntoView {
    view! {<section class="import-summary" aria-labelledby="import-summary-heading"><h2 id="import-summary-heading">"Review imported content"</h2>
    <ul>{review.with_untracked(|r|r.notes.clone()).into_iter().map(|note|view!{<li>{note}</li>}).collect_view()}</ul>
    {review.with_untracked(|r|!r.rejected_rows.is_empty()).then(||view!{<details><summary class="text-danger">"Rejected CSV rows"</summary><ul class="text-danger">{review.with_untracked(|r|r.rejected_rows.iter().take(20).cloned().collect::<Vec<_>>()).into_iter().map(|error|view!{<li>{format!("Row {}: {}",error["row"],error["message"].as_str().unwrap_or("Invalid row"))}</li>}).collect_view()}</ul><p>"Showing up to 20 rows. Download the report for all rejected rows."</p><ActionButton on_click=move |_|download(&json!(review.with_untracked(|r|r.rejected_rows.clone())),"import-errors",message)>"Download error report"</ActionButton></details>})}
    {review.with_untracked(|r|r.requires_acknowledgement).then(||view!{<Checkbox label="I reviewed the rejected or duplicate rows and accept these omissions." checked=Signal::derive(move||review.with(|r|r.acknowledged)) on_toggle=Callback::new(move|v|review.update(|r|r.acknowledged=v))/>})}
    <p>"This draft is kept only in this tab. Refreshing or closing it clears unsaved content."</p>
    </section>}
}

fn test_panel(
    api: Api,
    token: String,
    family: Family,
    editor: Editor,
    open: RwSignal<bool>,
) -> impl IntoView {
    let token = StoredValue::new(token);
    let profiles = RwSignal::new(Vec::<Value>::new());
    let pages = RwSignal::new(Vec::<Value>::new());
    let selected = RwSignal::new(String::new());
    let page = RwSignal::new(String::new());
    let recipient = RwSignal::new(String::new());
    let first = RwSignal::new("Alice".to_owned());
    let last = RwSignal::new("Example".to_owned());
    let position = RwSignal::new("Analyst".to_owned());
    let url = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let message = Message::new(String::new());
    spawn_local(async move {
        for (kind, signal) in [("smtp", profiles), ("pages", pages)] {
            match api.get(&format!("/api/{kind}/summary")).await {
                Ok(v) => signal.set(v["items"].as_array().cloned().unwrap_or_default()),
                Err(e) => message.error(e),
            }
        }
    });
    let close = move || {
        open.set(false);
        finish_edit(editor, family);
    };
    let send = move |connection: bool| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        message.clear();
        spawn_local(async move {
            if !connection
                && (editor.draft.with_untracked(ResourceDraft::dirty)
                    || editor
                        .draft
                        .with_untracked(|d| d.value()["id"].as_i64().unwrap_or(0) == 0))
                && !save_draft(api, token.get_value(), family, editor).await
            {
                message.error("The resource could not be saved. No test email was sent.".into());
                busy.set(false);
                return;
            }
            let smtp = if family == Family::Profiles {
                editor.draft.get_untracked().value().clone()
            } else {
                json!({"name":selected.get_untracked()})
            };
            let (path, payload) = if connection {
                ("/api/util/test_connection", smtp)
            } else {
                (
                    "/api/util/send_test_email",
                    json!({"smtp":smtp,"template":{"name":if family==Family::Templates{editor.text("name")}else{String::new()}},"page":{"name":page.get_untracked()},"url":url.get_untracked(),"email":recipient.get_untracked(),"first_name":first.get_untracked(),"last_name":last.get_untracked(),"position":position.get_untracked()}),
                )
            };
            match api
                .resource("POST", path, &token.get_value(), Some(&payload), None)
                .await
            {
                Ok(_) => message.status(
                    if connection {
                        "Connection and authentication verified. No email sent."
                    } else {
                        "Accepted by SMTP server. Inbox delivery is not confirmed."
                    }
                    .into(),
                ),
                Err(e) => message.error(e),
            }
            busy.set(false);
        });
    };
    view! {<Dialog title="Send a test email" busy=busy on_cancel=Callback::new(move |_|close())><div class="management-heading"><h2 id="test-heading">"Send a test email"</h2><ActionButton disabled=Signal::derive(move||busy.get()) on_click=move |_|close()>"Close"</ActionButton></div><p>"Test the selected saved content with one recipient. Saving and delivery are separate outcomes."</p>
    {(family==Family::Templates).then(||view!{<Select label="Sending profile" value=selected on_change=Callback::new(move|v|selected.set(v))><option value="">"Choose a profile"</option>{move||profiles.get().into_iter().map(|v|{let name=v["name"].as_str().unwrap_or("").to_owned();{let label=name.clone();view!{<option value=name>{label}</option>}}}).collect_view()}</Select>})}
    <ActionButton disabled=Signal::derive(move||busy.get()) on_click=move |_|send(true)>"Test connection only"</ActionButton><div class="resource-columns">
    {[ (recipient,"Recipient email"),(first,"First name"),(last,"Last name"),(position,"Position"),(url,"Public test URL (optional)") ].into_iter().map(|(signal,label)|view!{<label>{label}<TextInput value=Signal::derive(move||signal.get()) on_input=move|e|signal.set(event_target_value(&e))/></label>}).collect_view()}
    <Select label="Preview landing page" value=page on_change=Callback::new(move|v|page.set(v))><option value="">"No landing page"</option>{move||pages.get().into_iter().map(|v|{let name=v["name"].as_str().unwrap_or("").to_owned();{let label=name.clone();view!{<option value=name>{label}</option>}}}).collect_view()}</Select></div>
    {message.view()}<ActionButton style=ButtonStyle::Primary disabled=Signal::derive(move||busy.get()||recipient.get().is_empty()) on_click=move |_|send(false)>{move||if busy.get(){"Working…"}else if editor.draft.with(ResourceDraft::dirty)||editor.draft.with(|d|d.value()["id"].as_i64().unwrap_or(0)==0){"Save and send test"}else{"Send test email"}}</ActionButton>
    </Dialog>}
}
