use super::{ActionButton, ActionLink, ButtonStyle, ControlSize, DeleteConfirmation};
use crate::{account::Api, feedback::Message};
use leptos::prelude::*;
pub(crate) use ruphish_ui::deletion_state::DeleteTarget;
use ruphish_ui::deletion_state::Deletion;
use ruphish_ui::view_tasks::ViewTasks;
use ruphish_ui::{tr, trs};

/// Common resource actions. Feature-specific actions remain explicit children.
#[component]
pub(crate) fn ResourceActions(
    #[prop(into)] edit_href: String,
    can_edit: bool,
    #[prop(optional)] hide_edit: bool,
    on_delete: Callback<()>,
    #[prop(optional)] view_href: Option<String>,
    #[prop(default = ControlSize::Medium)] size: ControlSize,
    #[prop(into, optional)] disabled: Signal<bool>,
    #[prop(into, optional)] delete_disabled: Signal<bool>,
    #[prop(into, optional)] delete_reason: Signal<String>,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    view! {
        <div class="row-actions">
            {view_href.map(|href| view! { <ActionLink href=href size=size disabled=disabled>{tr!(ui.view)}</ActionLink> })}
            <ActionLink href=edit_href size=size hidden={!can_edit || hide_edit} disabled=disabled>{tr!(ui.edit)}</ActionLink>
            {children.map(|children| children())}
            <ActionButton style=ButtonStyle::Ghost size=size hidden=!can_edit
                disabled=Signal::derive(move || disabled.get() || delete_disabled.get())
                title=delete_reason on_click=move |_| on_delete.run(())>{tr!(ui.delete)}</ActionButton>
        </div>
    }
}

/// A pending confirmation owns a snapshot of the resource, including its revision.
/// Requests belong to the page, so closing/re-rendering a dialog cannot cancel them.
#[derive(Clone, Copy)]
pub(crate) struct ResourceDeletion {
    state: RwSignal<Deletion>,
    pub busy: Signal<bool>,
    message: Message,
    tasks: ViewTasks,
}
impl ResourceDeletion {
    pub fn new() -> Self {
        let state = RwSignal::new(Deletion::default());
        Self {
            state,
            busy: Signal::derive(move || state.with(Deletion::busy)),
            message: Message::new(String::new()),
            tasks: ViewTasks::default(),
        }
    }
    pub fn select(self, target: DeleteTarget) {
        if !self.busy.get_untracked() {
            self.message.clear();
            self.state.update(|state| state.select(target));
        }
    }
}

#[component]
pub(crate) fn ResourceDeleteDialog(
    deletion: ResourceDeletion,
    api: Api,
    token: String,
    on_deleted: Callback<i64>,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let token = StoredValue::new(token);
    let target = Memo::new(move |_| deletion.state.with(|state| state.target().cloned()));
    let confirm = Callback::new(move |()| {
        let Some(target) = deletion.state.try_update(Deletion::begin).flatten() else {
            return;
        };
        deletion.message.clear();
        deletion.tasks.spawn(async move {
            let result = api
                .command(
                    "DELETE",
                    &target.path,
                    &token.get_value(),
                    None,
                    target.revision.as_deref(),
                )
                .await;
            let deleted = deletion
                .state
                .try_update(|state| state.finish(result.is_ok()))
                .flatten();
            match result {
                Ok(_) => {
                    if let Some(id) = deleted {
                        on_deleted.run(id);
                    }
                }
                Err(error) => deletion.message.error(error),
            }
        });
    });
    view! {
        {move || target.get().map(|target| view! {
            <DeleteConfirmation title=leptos_i18n::t_string!(i18n, messages.delete_arg0, arg0 = (target.name).to_string())
                description=target.description confirm_label=trs!(ui.delete)
                busy=deletion.busy message=deletion.message on_confirm=confirm
                on_cancel=Callback::new(move |()| {
                    deletion.state.update(Deletion::cancel);
                })/>
        })}
    }
}
