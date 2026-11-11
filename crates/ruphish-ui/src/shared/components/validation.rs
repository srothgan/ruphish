//! Accessible form feedback; only changed field diagnostics notify their controls.
use crate::api::error::ApiError;
use crate::shared::forms::state::FormState;
use leptos::prelude::*;
use ruphish_contracts::validation::Issue;
use ruphish_domain::validation::{Code, Report};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicUsize, Ordering},
};

static CONTROL_ID: AtomicUsize = AtomicUsize::new(0);
#[derive(Clone, Copy)]
struct Control {
    error: RwSignal<Option<Code>>,
    focus: Callback<()>,
}
type Controls = BTreeMap<String, BTreeMap<String, Control>>;

#[derive(Clone, Copy)]
pub(crate) struct FormValidation {
    state: StoredValue<FormState>,
    local: Memo<Report>,
    visible: RwSignal<Vec<Issue>>,
    controls: StoredValue<Controls>,
    reveal: StoredValue<Option<Callback<String>>>,
}
impl FormValidation {
    pub(crate) fn new(validate: impl Fn() -> Report + Send + Sync + 'static) -> Self {
        let form = Self {
            state: StoredValue::new(FormState::default()),
            local: Memo::new(move |_| validate()),
            visible: RwSignal::new(Vec::new()),
            controls: StoredValue::new(BTreeMap::new()),
            reveal: StoredValue::new(None),
        };
        Effect::new(move |_| {
            form.local.track();
            form.refresh();
        });
        provide_context(form);
        form
    }
    // Touched state and request generations are not UI-wide reactive dependencies.
    // Publish only changed errors, rather than waking every input after each key/blur.
    fn refresh(self) {
        let next = self.state.with_value(|state| {
            self.local
                .with_untracked(|local| state.issues(local.issues()))
        });
        let previous = self.visible.get_untracked();
        if next == previous {
            return;
        }
        let fields: BTreeSet<_> = previous
            .iter()
            .chain(&next)
            .map(|issue| issue.field.as_str())
            .collect();
        self.controls.with_value(|controls| {
            for field in fields {
                let before = previous
                    .iter()
                    .find(|issue| issue.field == field)
                    .map(|issue| issue.code);
                let after = next
                    .iter()
                    .find(|issue| issue.field == field)
                    .map(|issue| issue.code);
                if before != after
                    && let Some(controls) = controls.get(field)
                {
                    for control in controls.values() {
                        control.error.set(after);
                    }
                }
            }
        });
        self.visible.set(next);
    }
    pub(crate) fn on_reveal(self, callback: Callback<String>) {
        self.reveal.set_value(Some(callback));
    }
    /// Let grouped controls reveal themselves when they contain a visible error.
    pub(crate) fn error(self, field: &str) -> Option<Code> {
        self.visible.with(|issues| {
            issues
                .iter()
                .find(|issue| issue.field == field)
                .map(|issue| issue.code)
        })
    }
    pub(crate) fn check(self) -> bool {
        self.check_report(&self.local.get_untracked())
    }
    /// Validate a navigation destination, revealing errors outside the currently visible step.
    pub(crate) fn check_report(self, report: &Report) -> bool {
        self.state
            .update_value(|state| state.submit(report.issues()));
        self.refresh();
        if let Some(issue) = report.issues().first() {
            self.focus_field(issue.field.clone());
            false
        } else {
            true
        }
    }
    pub(crate) fn edited(self, field: &str) {
        self.state.update_value(|state| state.edit(field));
        self.refresh();
    }
    pub(crate) fn reset(self) {
        self.state.update_value(FormState::reset);
        self.refresh();
    }
    fn touched(self, field: &str) {
        self.state.update_value(|state| state.touch(field));
        self.refresh();
    }
    pub(crate) fn ticket(self) -> u64 {
        self.state.with_value(FormState::ticket)
    }
    pub(crate) fn server(self, ticket: u64, error: &ApiError) {
        if let Some(report) = &error.validation {
            let mut accepted = false;
            self.state
                .update_value(|state| accepted = state.receive(ticket, report.clone()));
            if accepted {
                self.refresh();
                self.focus();
            }
        }
    }
    pub(crate) fn focus(self) {
        self.refresh();
        if let Some(issue) = self
            .visible
            .with_untracked(|issues| issues.first().cloned())
        {
            self.focus_field(issue.field);
        }
    }
    fn focus_field(self, field: String) {
        if let Some(reveal) = self.reveal.get_value() {
            reveal.run(field.clone());
        }
        let ticket = self.ticket();
        leptos::leptos_dom::helpers::request_animation_frame(move || {
            if self.state.try_with_value(FormState::ticket) != Some(ticket) {
                return;
            }
            let _ = self.controls.try_with_value(|controls| {
                if let Some(control) = controls
                    .get(&field)
                    .and_then(|controls| controls.values().next())
                {
                    control.focus.run(());
                }
            });
        });
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ValidationBinding {
    field: StoredValue<String>,
    id: StoredValue<String>,
    error: RwSignal<Option<Code>>,
    form: Option<FormValidation>,
}
impl ValidationBinding {
    pub(crate) fn new(field: String) -> Self {
        let form = (!field.is_empty())
            .then(use_context::<FormValidation>)
            .flatten();
        let error = form.and_then(|form| {
            form.state.with_value(|state| {
                form.local
                    .with_untracked(|local| state.field(local.issues(), &field))
            })
        });
        Self {
            field: StoredValue::new(field),
            id: StoredValue::new(format!(
                "field-error-{}",
                CONTROL_ID.fetch_add(1, Ordering::Relaxed)
            )),
            error: RwSignal::new(error),
            form,
        }
    }
    pub(crate) fn register(self, focus: Callback<()>) {
        if let Some(form) = self.form {
            let field = self.field.get_value();
            let id = self.id.get_value();
            form.controls.update_value(|controls| {
                controls.entry(field.clone()).or_default().insert(
                    id.clone(),
                    Control {
                        error: self.error,
                        focus,
                    },
                );
            });
            on_cleanup(move || {
                let _ = form.controls.try_update_value(|controls| {
                    if let Some(entries) = controls.get_mut(&field) {
                        entries.remove(&id);
                        if entries.is_empty() {
                            controls.remove(&field);
                        }
                    }
                });
            });
        }
    }
    pub(crate) fn edited(self) {
        if let Some(form) = self.form {
            self.field.with_value(|field| form.edited(field));
        }
    }
    pub(crate) fn touched(self) {
        if let Some(form) = self.form {
            self.field.with_value(|field| form.touched(field));
        }
    }
    pub(crate) fn invalid(self) -> bool {
        self.error.get().is_some()
    }
    pub(crate) fn described_by(self) -> Option<String> {
        self.invalid().then(|| self.id.get_value())
    }
    pub(crate) fn view(self) -> impl IntoView {
        let i18n = crate::localization::use_i18n();
        view! {
            <Show when=move || self.invalid()>
                <span class="field-error" id=self.id.get_value()>
                    {move || {
                        self.error
                            .get()
                            .map(|code| crate::localization::validation_label(i18n, code))
                    }}
                </span>
            </Show>
        }
    }
}

#[component]
pub(crate) fn ValidationSummary(form: FormValidation) -> impl IntoView {
    let i18n = crate::localization::use_i18n();
    view! {
        <div class="validation-summary" role="alert" aria-atomic="true">
            <Show when=move || !form.visible.with(Vec::is_empty)>
                <p>{move || leptos_i18n::t_string!(i18n, validation.summary)}</p>
                <ul>
                    {move || {
                        form.visible
                            .get()
                            .into_iter()
                            .filter(|issue| {
                                !form
                                    .controls
                                    .with_value(|controls| controls.contains_key(&issue.field))
                            })
                            .map(|issue| {
                                view! {
                                    <li>
                                        {crate::localization::validation_label(i18n, issue.code)}
                                    </li>
                                }
                            })
                            .collect_view()
                    }}
                </ul>
            </Show>
        </div>
    }
}

/// Diagnostic anchor for grouped or JavaScript-owned controls.
#[component]
pub(crate) fn ValidationAnchor(#[prop(into)] field: String) -> impl IntoView {
    let binding = ValidationBinding::new(field);
    let node = NodeRef::<leptos::html::Div>::new();
    binding.register(Callback::new(move |()| {
        if let Some(node) = node.get() {
            let _ = node.focus();
        }
    }));
    view! {
        <div node_ref=node tabindex="-1" aria-describedby=move || binding.described_by()>
            {binding.view()}
        </div>
    }
}
