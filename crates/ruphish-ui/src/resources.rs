//! Resource lists and full-page editors using the preserved HTTP boundary.
use crate::account::Api;
use crate::components::{
    DataTable, DeleteConfirmation, EmptyState, Field, FormFields, FormFooter, ManagementPage,
    PageHeader, SaveButton, modal_node,
};
use crate::feedback::Message;
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
                view!{<ManagementPage><h1>"This draft is no longer available"</h1><p class="form-message error" role="alert">{ImportError::DraftUnavailable.to_string()}</p><p>"Imported drafts stay in this tab until saved or discarded. Refreshing, closing the tab or changing accounts clears them."</p><a class="button primary" href=format!("/{}?import=1",family.route())>"Import again"</a></ManagementPage>}.into_any()
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
    let deleting = RwSignal::new(None::<Value>);
    let tasks = ruphish_ui::view_tasks::ViewTasks::default();
    let busy = RwSignal::new(false);
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
    let remove = move |_| {
        if busy.get_untracked() {
            return;
        }
        let Some(value) = deleting.get_untracked() else {
            return;
        };
        generation.update_value(|n| *n += 1);
        loading.set(false);
        busy.set(true);
        tasks.spawn(async move {
            let path = format!("/api/{}/{}", family.route(), value["id"]);
            match api
                .resource("DELETE", &path, &token.get_value(), None, None)
                .await
            {
                Ok(_) => {
                    rows.update(|rows| rows.retain(|row| row["id"] != value["id"]));
                    deleting.set(None);
                    refresh.update(|v| *v += 1);
                    message.status("Resource deleted.".into());
                }
                Err(e) => message.error(e),
            }
            busy.set(false);
        });
    };
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
        <button hidden=!can_edit on:click=move |_|import_open.set(true)>"Import"</button><a hidden=!can_edit class="button primary" href=format!("/{}/new",family.route())>{format!("Create {}",family.singular())}</a>
    </PageHeader>
    {move||(can_edit && import_open.get()).then(||import_panel(api,token.get_value(),family,import_open))}
    {message.view()}
    {move||deleting.get().map(|v|view!{<DeleteConfirmation title=format!("Delete {}?",v["name"].as_str().unwrap_or("resource")) description="This permanently removes the saved resource. Resources used by unfinished campaigns cannot be deleted." confirm_label="Delete resource" busy=busy on_confirm=Callback::new(remove) on_cancel=Callback::new(move|()|deleting.set(None)) message=message/>})}
    <div class="management-toolbar"><Field label="Search by name"><input type="search" prop:value=move||search.get() on:input=move|e|search.set(event_target_value(&e))/></Field><span>{move||format!("{} resources",visible.with(Vec::len))}</span></div>
    <Show when=move||loading.get()><p role="status">"Loading resources..."</p></Show>
    <Show when=move||!loading.get()&&failed.get()><button on:click=move |_|refresh.update(|v|*v+=1)>"Retry loading resources"</button></Show>
    <DataTable label=family.title() headers=vec!["Name",if family==Family::Groups{"Recipients"}else{"Active campaigns"},"Last modified","Actions"]>
    {move||visible.get().into_iter().map(|v|{let id=v["id"].as_i64().unwrap_or(0);let name=v["name"].as_str().unwrap_or("Unnamed").to_owned();let delete_value=v.clone();view!{<tr><th scope="row"><a href=format!("/{}/{id}",family.route())>{name}</a></th><td>{v[if family==Family::Groups{"num_targets"}else{"active_campaigns"}].as_i64().unwrap_or(0)}</td><td>{v["modified_date"].as_str().unwrap_or("").to_owned()}</td><td><div class="row-actions"><a href=if can_edit{format!("/{}/{id}/edit",family.route())}else{format!("/{}/{id}",family.route())}>{if can_edit{"Edit"}else{"View"}}</a><a hidden=!can_edit href=format!("/{}/new?duplicate={id}",family.route())>"Duplicate"</a><button hidden=!can_edit class="text-danger" on:click=move |_|{message.clear();deleting.set(Some(delete_value.clone()))}>"Delete"</button></div></td></tr>}}).collect_view()}
    </DataTable>
    <Show when=move||!loading.get()&&!failed.get()&&visible.with(Vec::is_empty)>
        <EmptyState title="No resources found"><p>{move||if search.get().is_empty(){"Create a resource or import a file to review in a new draft."}else{"Try another search."}}</p></EmptyState>
    </Show>
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
        view! {<Field label=label><input id=key name=key type=input_type autocomplete="off" prop:value=move||self.text(key) on:input=move|e|self.set(key,json!(event_target_value(&e)))/></Field>}
    }
    fn flag(self, key: &'static str, label: &'static str) -> impl IntoView {
        view! {<Field label=label checkbox=true><input type="checkbox" prop:checked=move||self.draft.with(|d|d.value()[key].as_bool().unwrap_or(false)) on:change=move|e|self.set(key,json!(event_target_checked(&e)))/></Field>}
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
    view! {<ManagementPage><a class="breadcrumb" href=format!("/{}",family.route())>{format!("Back to {}",family.title().to_lowercase())}</a>
    <PageHeader title=Signal::derive(move||{let name=editor.text("name");if name.is_empty(){format!("New {}",family.singular())}else{name}})
        status=true subtitle=Signal::derive(move||if editor.busy.get(){"Saving or checking..."}else if loading.get(){"Loading resource..."}else if load_failed.get(){"Resource could not be loaded"}else if editor.readonly{"Read-only access"}else if editor.visual_pending.get(){"Apply or cancel visual edits before saving"}else if editor.draft.with(ResourceDraft::dirty){"Unsaved changes"}else if editor.draft.with(|d|d.value()["id"].as_i64().unwrap_or(0)==0){"Not saved yet"}else{"No unsaved changes"}.to_owned())>
    {(family==Family::Templates||family==Family::Profiles).then(||view!{<button disabled=move||loading.get()||load_failed.get()||editor.readonly||editor.busy.get()||editor.visual_pending.get() on:click=move |_|test_open.set(true)>"Send test"</button>})}
    <SaveButton disabled=Signal::derive(move||loading.get()||load_failed.get()||editor.readonly||editor.busy.get()||editor.visual_pending.get()||editor.import_review.is_some_and(|r|!r.with(ImportReview::ready))) on_click=Callback::new(save) label=Signal::derive(move||if editor.draft.with(|d|d.value()["id"].as_i64().unwrap_or(0)==0){format!("Create {}",family.singular())}else{"Save changes".into()})/>
    </PageHeader>
    {matches!(family,Family::Templates|Family::Pages|Family::Profiles).then(||view!{<p class="resource-usage">"Campaigns use the current saved version of this resource. Editing it can affect unfinished campaigns."</p>})}
    {editor.import_review.map(|review|import_review_panel(review,editor.message))}
    {editor.message.view()}<Show when=move||!loading.get()&&load_failed.get()><button on:click=move |_|reload.update(|n|*n+=1)>"Retry loading resource"</button></Show><Show when=move||loading.get()><p>"Loading resource..."</p></Show>
    <FormFields disabled=Signal::derive(move||load_failed.get()||editor.readonly||loading.get()||editor.busy.get())>{editor.field("name","Name","text")}{content}</FormFields>
    <FormFooter><button class="text-danger" disabled=move||loading.get()||load_failed.get()||editor.busy.get() on:click=move |_|{editor.draft.update(ResourceDraft::cancel);editor.epoch.update(|v|*v+=1);let id=editor.draft.with_untracked(|d|d.value()["id"].as_i64().unwrap_or(0)); editor.replace_url.run(if id>0{format!("/{}/{id}",family.route())}else{format!("/{}",family.route())});}>"Cancel changes"</button></FormFooter>
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
    view! {<section class="resource-section"><div class="management-heading"><h2>"Recipients"</h2><button type="button" on:click=move |_|editor.draft.update(|d|d.edit(|v|{if let Some(rows)=v["targets"].as_array_mut(){rows.push(json!({"email":"","first_name":"","last_name":"","position":""}));}}))>"Add recipient"</button></div><DataTable label="Recipients" headers=vec!["Email","First name","Last name","Position","Action"] class="recipient-table">
    <For each=move||0..editor.draft.with(|d|d.value()["targets"].as_array().map_or(0,Vec::len)) key=|i|*i children=move|index|view!{<tr>{["email","first_name","last_name","position"].into_iter().map(move|key|view!{<td><input aria-label=format!("Recipient {} {key}",index+1) prop:value=move||editor.draft.with(|d|d.value()["targets"][index][key].as_str().unwrap_or("").to_owned()) on:input=move|e|editor.draft.update(|d|d.edit(|v|v["targets"][index][key]=json!(event_target_value(&e))))/></td>}).collect_view()}<td><button type="button" on:click=move |_|editor.draft.update(|d|d.edit(|v|{if let Some(rows)=v["targets"].as_array_mut() && index<rows.len(){rows.remove(index);}}))>"Remove"</button></td></tr>}/>
    </DataTable></section>}
}
fn content_workspace(
    api: Api,
    token: StoredValue<String>,
    editor: Editor,
    family: Family,
) -> impl IntoView {
    use ruphish_ui::preview_state::ContentPreview;
    let showing_preview = RwSignal::new(false);
    let preview = RwSignal::new(ContentPreview::default());
    let html_source = Memo::new(move |_| editor.text("html"));
    let retry = RwSignal::new(0u64);
    let edit_tab = NodeRef::<html::Button>::new();
    let preview_tab = NodeRef::<html::Button>::new();
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
    let keyboard = move |event: web_sys::KeyboardEvent| {
        let next = match event.key().as_str() {
            "ArrowLeft" | "ArrowRight" => !showing_preview.get_untracked(),
            "Home" => false,
            "End" => true,
            _ => return,
        };
        event.prevent_default();
        if next && editor.visual_pending.get_untracked() {
            return;
        }
        showing_preview.set(next);
        let node = if next {
            preview_tab.get()
        } else {
            edit_tab.get()
        };
        if let Some(node) = node {
            let _ = node.focus();
        }
    };
    view! {<section class="content-workspace">
        <div class="content-heading"><h2>{if family==Family::Templates {"Email HTML"} else {"Landing-page HTML"}}</h2>
        <div class="content-tabs" role="tablist" aria-label="HTML editing and preview" on:keydown=keyboard>
            <button node_ref=edit_tab id="content-edit-tab" type="button" role="tab" aria-controls="content-edit-panel" aria-selected=move||(!showing_preview.get()).to_string() tabindex=move||if showing_preview.get(){-1}else{0} on:click=move |_|showing_preview.set(false)>"Edit"</button>
            <button node_ref=preview_tab id="content-preview-tab" type="button" role="tab" aria-controls="content-preview-panel" aria-selected=move||showing_preview.get().to_string() tabindex=move||if showing_preview.get(){0}else{-1} disabled=move||editor.visual_pending.get() on:click=move |_|showing_preview.set(true)>"Preview"</button>
        </div></div>
        <p class="content-tab-help" hidden=move||!editor.visual_pending.get()>"Apply or cancel visual edits before previewing."</p>
        <div id="content-edit-panel" role="tabpanel" aria-labelledby="content-edit-tab" hidden=move||showing_preview.get()>
            {crate::editor::content_editor(Signal::derive(move||editor.text("html")),Callback::new(move|value|editor.set("html",json!(value))),editor.epoch,editor.message,editor.visual_pending,editor.readonly)}
        </div>
        <div id="content-preview-panel" class="content-preview-panel" role="tabpanel" aria-labelledby="content-preview-tab" hidden=move||!showing_preview.get() aria-busy=move||preview.with(|p|p.loading).to_string()>
            <p class="content-tab-help">"Current draft with sample recipient Alice Example. Scripts, external resources and submissions are blocked."{(family==Family::Templates).then_some(" Email clients may render it differently.")}</p>
            <Show when=move||preview.with(|p|p.loading)><p role="status">"Rendering preview..."</p></Show>
            {move||preview.get().error.map(|error|view!{<p class="form-message error" role="alert">{error}</p><button type="button" on:click=move |_|retry.update(|v|*v+=1)>"Retry preview"</button>})}
            {move||preview.get().html.map(|html|view!{<iframe title="Current HTML draft preview" sandbox="" referrerpolicy="no-referrer" srcdoc=preview_document(&html)></iframe>})}
        </div>
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
    {(family==Family::Templates).then(||view!{<label class="text-alternative">"Plain-text alternative"<textarea rows="6" prop:value=move||editor.text("text") on:input=move|e|editor.set("text",json!(event_target_value(&e)))></textarea></label><div class="resource-section"><h2>"Attachments"</h2><input node_ref=files hidden type="file" multiple on:change=add/><button type="button" on:click=move |_|{if let Some(node)=files.get(){node.click();}}>"Add attachments"</button><ul>{move||editor.draft.with(|d|d.value()["attachments"].as_array().cloned().unwrap_or_default()).into_iter().enumerate().map(|(i,a)|view!{<li>{a["name"].as_str().unwrap_or("").to_owned()}<button type="button" on:click=move |_|editor.draft.update(|d|{d.remove_attachment(i);})>"Remove"</button></li>}).collect_view()}</ul></div>})}
    {(family==Family::Pages).then(||view!{<section class="resource-section"><h2>"Optional campaign defaults"</h2><p>"Public pages preserve field names and styling, but block authored scripts, embedded frames and external resources. Use embedded PNG, JPEG or GIF images. Preview the final page before launch; form submissions send only event metadata and optional password presence."</p><p>"Suggested submission settings for campaigns using this page. Leave tracking off and the redirect blank if no defaults are needed."</p>{editor.flag("capture_credentials","Record form submission")}{editor.flag("capture_passwords","Record password presence")}{editor.field("redirect_url","Redirect URL after submission (optional)","text")}<p>"Track only whether fields were filled and submitted. Entered usernames, passwords and other field values are never collected."</p></section>})}
    </section>}
}
fn profile_fields(editor: Editor) -> impl IntoView {
    view! {<section class="resource-section"><h2>"Connection"</h2><div class="resource-columns">{editor.field("host","SMTP host and port","text")}{editor.field("from_address","Sender address","text")}{editor.field("username","Username (optional)","text")}{editor.field("password","Password","password")}</div><p>"Port defaults to 25. Port 465 uses implicit TLS; STARTTLS is used when advertised. Passwords are masked, never replaced with a placeholder."</p>{editor.flag("ignore_cert_errors","Ignore SMTP certificate errors")}</section><section class="resource-section"><div class="management-heading"><h2>"Additional headers"</h2><button type="button" on:click=move |_|editor.draft.update(|d|d.edit(|v|{if let Some(rows)=v["headers"].as_array_mut(){rows.push(json!({"key":"","value":""}));}}))>"Add header"</button></div><For each=move||0..editor.draft.with(|d|d.value()["headers"].as_array().map_or(0,Vec::len)) key=|i|*i children=move|index|view!{<div class="header-row">{["key","value"].into_iter().map(move|key|view!{<label>{if key=="key"{"Header name"}else{"Value"}}<input prop:value=move||editor.draft.with(|d|d.value()["headers"][index][key].as_str().unwrap_or("").to_owned()) on:input=move|e|editor.draft.update(|d|d.edit(|v|v["headers"][index][key]=json!(event_target_value(&e))))/></label>}).collect_view()}<button type="button" on:click=move |_|editor.draft.update(|d|d.edit(|v|{if let Some(rows)=v["headers"].as_array_mut() && index<rows.len(){rows.remove(index);}}))>"Remove"</button></div>}/></section>}
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
    let dialog = modal_node();
    view! {<dialog node_ref=dialog class="management-dialog" aria-labelledby="import-heading" on:cancel=move|e:web_sys::Event|{e.prevent_default();open.set(false);}>
        <div class="management-heading"><h2 id="import-heading">{format!("Import {}",family.singular())}</h2></div>
        <p>"Load content into a new Create form to review and finish. Nothing is saved yet."</p>
        <fieldset disabled=move||busy.get() class="import-fields">
        <input hidden node_ref=file type="file" accept=match family{Family::Groups=>".json,.csv",Family::Templates=>".json,.eml,.html,.htm",Family::Pages=>".json,.html,.htm",Family::Profiles=>".json"} on:change=read/>
        <button on:click=move |_|{if let Some(file)=file.get(){file.click();}}>"Choose file"</button>
        <label>"Import format"<select prop:value=move||mode.get() on:change=move|e|{mode.set(event_target_value(&e));content.set(String::new());message.clear();}>
        <option value="json">"Resource JSON"</option>
        {(family==Family::Groups).then(||view!{<option value="csv">"Recipients CSV"</option>})}
        {(family==Family::Templates).then(||view!{<option value="email">"Email (.eml)"</option>})}
        {matches!(family,Family::Templates|Family::Pages).then(||view!{<option value="html">"HTML document"</option>})}
        {(family==Family::Pages).then(||view!{<option value="site">"Website URL"</option>})}
        </select></label>
        <label>{move||if mode.get()=="site"{"Website URL"}else{"File content or pasted content"}}<textarea rows="7" prop:value=move||content.get() on:input=move|e|content.set(event_target_value(&e))></textarea></label>
        <Show when=move||mode.get()=="csv"><div class="resource-columns">{["Email column","First-name column","Last-name column","Position column"].into_iter().enumerate().map(|(i,label)|view!{<label>{label}<input prop:value=move||columns.get()[i].clone() on:input=move|e|columns.update(|v|v[i]=event_target_value(&e))/></label>}).collect_view()}</div></Show>
        <Show when=move||mode.get()=="email"><label class="checkbox-label"><input type="checkbox" prop:checked=move||convert.get() on:change=move|e|convert.set(event_target_checked(&e))/>"Replace links with the recipient URL"</label></Show>
        </fieldset>{message.view()}
        <div class="import-actions"><button class="text-danger" on:click=move |_|open.set(false)>"Cancel import"</button><button class="primary" disabled=move||busy.get()||content.get().trim().is_empty() on:click=review>{move||if busy.get(){"Reading import..."}else{"Continue to create"}}</button></div>
    </dialog>}
}

fn import_review_panel(review: RwSignal<ImportReview>, message: Message) -> impl IntoView {
    view! {<section class="import-summary" aria-labelledby="import-summary-heading"><h2 id="import-summary-heading">"Review imported content"</h2>
    <ul>{review.with_untracked(|r|r.notes.clone()).into_iter().map(|note|view!{<li>{note}</li>}).collect_view()}</ul>
    {review.with_untracked(|r|!r.rejected_rows.is_empty()).then(||view!{<details><summary class="text-danger">"Rejected CSV rows"</summary><ul class="text-danger">{review.with_untracked(|r|r.rejected_rows.iter().take(20).cloned().collect::<Vec<_>>()).into_iter().map(|error|view!{<li>{format!("Row {}: {}",error["row"],error["message"].as_str().unwrap_or("Invalid row"))}</li>}).collect_view()}</ul><p>"Showing up to 20 rows. Download the report for all rejected rows."</p><button on:click=move |_|download(&json!(review.with_untracked(|r|r.rejected_rows.clone())),"import-errors",message)>"Download error report"</button></details>})}
    {review.with_untracked(|r|r.requires_acknowledgement).then(||view!{<label class="checkbox-label"><input type="checkbox" prop:checked=move||review.with(|r|r.acknowledged) on:change=move|e|review.update(|r|r.acknowledged=event_target_checked(&e))/>"I reviewed the rejected or duplicate rows and accept these omissions."</label>})}
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
    let dialog = modal_node();
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
    view! {<dialog node_ref=dialog class="management-dialog" aria-labelledby="test-heading" on:cancel=move|e: web_sys::Event|{e.prevent_default();if !busy.get_untracked(){close();}}><div class="management-heading"><h2 id="test-heading">"Send a test email"</h2><button disabled=move||busy.get() on:click=move |_|close()>"Close"</button></div><p>"Test the selected saved content with one recipient. Saving and delivery are separate outcomes."</p>
    {(family==Family::Templates).then(||view!{<label>"Sending profile"<select prop:value=move||selected.get() on:change=move|e|selected.set(event_target_value(&e))><option value="">"Choose a profile"</option>{move||profiles.get().into_iter().map(|v|{let name=v["name"].as_str().unwrap_or("").to_owned();{let label=name.clone();view!{<option value=name>{label}</option>}}}).collect_view()}</select></label>})}
    <button disabled=move||busy.get() on:click=move |_|send(true)>"Test connection only"</button><div class="resource-columns">
    {[ (recipient,"Recipient email"),(first,"First name"),(last,"Last name"),(position,"Position"),(url,"Public test URL (optional)") ].into_iter().map(|(signal,label)|view!{<label>{label}<input prop:value=move||signal.get() on:input=move|e|signal.set(event_target_value(&e))/></label>}).collect_view()}
    <label>"Preview landing page"<select prop:value=move||page.get() on:change=move|e|page.set(event_target_value(&e))><option value="">"No landing page"</option>{move||pages.get().into_iter().map(|v|{let name=v["name"].as_str().unwrap_or("").to_owned();{let label=name.clone();view!{<option value=name>{label}</option>}}}).collect_view()}</select></label></div>
    {message.view()}<button class="primary" disabled=move||busy.get()||recipient.get().is_empty() on:click=move |_|send(false)>{move||if busy.get(){"Working..."}else if editor.draft.with(ResourceDraft::dirty)||editor.draft.with(|d|d.value()["id"].as_i64().unwrap_or(0)==0){"Save and send test"}else{"Send test email"}}</button>
    </dialog>}
}
