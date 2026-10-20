use crate::feedback::Message;
use leptos::prelude::*;

#[derive(Clone, Copy, Default)]
pub(crate) enum ButtonStyle {
    /// Neutral action: the common default.
    #[default]
    Secondary,
    /// The one primary action in a view (save, create, confirm).
    Primary,
    /// Destructive confirmation (delete, replace) — the only red button.
    Danger,
    /// Quiet dismiss (cancel, close, sign out). Reads as a ghost, not a warning.
    Ghost,
}
impl ButtonStyle {
    pub(super) fn class(self) -> &'static str {
        match self {
            Self::Secondary => "",
            Self::Primary => "primary",
            Self::Danger => "danger",
            Self::Ghost => "ghost",
        }
    }
}

/// Control sizing shared by buttons so in-row actions stay compact without ad hoc padding.
#[derive(Clone, Copy, Default)]
pub(crate) enum ControlSize {
    #[default]
    Medium,
    Small,
}
impl ControlSize {
    fn class(self) -> &'static str {
        match self {
            Self::Medium => "",
            Self::Small => "control-sm",
        }
    }
}

#[component]
pub(crate) fn ActionButton(
    children: Children,
    #[prop(default = ButtonStyle::Secondary)] style: ButtonStyle,
    #[prop(default = ControlSize::Medium)] size: ControlSize,
    #[prop(default = "button")] kind: &'static str,
    #[prop(into, optional)] disabled: Signal<bool>,
    #[prop(into, optional)] hidden: Signal<bool>,
    #[prop(into, optional)] title: Signal<String>,
    #[prop(into, optional)] busy: Signal<String>,
    #[prop(into, optional)] pressed: Signal<String>,
    #[prop(default = false)] autofocus: bool,
    #[prop(default = false)] close_panel: bool,
    #[prop(optional_no_strip)] form: Option<&'static str>,
    #[prop(into, optional)] on_click: Option<Callback<leptos::ev::MouseEvent>>,
) -> impl IntoView {
    view! {
        <button type=kind class=format!("{} {}", style.class(), size.class()) form=form autofocus=autofocus
            data-panel-close=close_panel.then_some("")
            disabled=move || disabled.get() hidden=move || hidden.get()
            title=move || title.get()
            aria-busy=move || (!busy.get().is_empty()).then(|| busy.get())
            aria-pressed=move || (!pressed.get().is_empty()).then(|| pressed.get())
            on:click=move |event| { if !disabled.get_untracked() && let Some(action) = on_click { action.run(event); } }>
            {children()}
        </button>
    }
}

#[component]
pub(crate) fn ActionLink(
    children: Children,
    #[prop(into)] href: String,
    #[prop(default = ButtonStyle::Secondary)] style: ButtonStyle,
    #[prop(default = ControlSize::Medium)] size: ControlSize,
    #[prop(into, optional)] hidden: Signal<bool>,
) -> impl IntoView {
    view! { <a class=format!("button {} {}", style.class(), size.class()) href=href hidden=move || hidden.get()>{children()}</a> }
}

/// Copy a value without placing long verification tokens into the page layout.
#[component]
pub(crate) fn CopyButton(label: &'static str, value: String, message: Message) -> impl IntoView {
    use leptos::leptos_dom::helpers::{TimeoutHandle, set_timeout_with_handle};
    let copied = RwSignal::new(false);
    let copying = RwSignal::new(false);
    let reset = StoredValue::new(None::<TimeoutHandle>);
    on_cleanup(move || {
        reset.update_value(|timer| {
            if let Some(timer) = timer.take() {
                timer.clear();
            }
        });
    });
    view! {
        <button type="button" class="copy-button" title=format!("Copy {label}") disabled=move||copying.get() on:click=move |_|{
            if copying.get_untracked() { return; }
            reset.update_value(|timer| {
                if let Some(timer) = timer.take() { timer.clear(); }
            });
            copied.set(false);
            copying.set(true);
            let value=value.clone();
            leptos::task::spawn_local_scoped_with_cancellation(async move {
                if crate::account::copy_text(&value).await.is_ok() {
                    copied.set(true);
                    reset.set_value(set_timeout_with_handle(move||copied.set(false), std::time::Duration::from_secs(2)).ok());
                } else {message.error(format!("Could not copy {label}."))}
                copying.set(false);
            });
        }><svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true" focusable="false"><rect x="8" y="8" width="12" height="12" rx="2"/><path d="M15 8V4H4v11h4"/></svg>
            <span class="copy-button-label"><span class="copy-button-size" aria-hidden="true">{format!("Copy {label}")}</span><span role="status">{move||if copying.get(){"Copying…".to_owned()}else if copied.get(){"Copied".to_owned()}else{format!("Copy {label}")}}</span></span>
        </button>
    }
}

#[component]
pub(crate) fn SaveButton(
    #[prop(into)] label: Signal<String>,
    #[prop(into)] disabled: Signal<bool>,
    #[prop(default = "button")] kind: &'static str,
    #[prop(optional)] form: Option<&'static str>,
    #[prop(optional)] on_click: Option<Callback<leptos::ev::MouseEvent>>,
) -> impl IntoView {
    view! { <ActionButton style=ButtonStyle::Primary kind=kind form=form disabled=disabled
    on_click=move |event| {if let Some(action) = on_click {action.run(event)}}>{move || label.get()}</ActionButton> }
}
