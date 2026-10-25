//! Accessible form feedback; feature modules supply the authoritative validator.
use leptos::prelude::*;
use ruphish_domain::validation::{Code, Report};
use ruphish_ui::{api_error::ApiError, form_state::FormState};
use std::sync::atomic::{AtomicUsize, Ordering};

static CONTROL_ID: AtomicUsize = AtomicUsize::new(0);
type Controls = Vec<(String, String, Callback<()>)>;

#[derive(Clone, Copy)]
pub(crate) struct FormValidation {
    state: RwSignal<FormState>,
    local: Memo<Report>,
    controls: StoredValue<Controls>,
    reveal: StoredValue<Option<Callback<String>>>,
}
impl FormValidation {
    pub(crate) fn new(validate: impl Fn() -> Report + Send + Sync + 'static) -> Self {
        let form = Self {
            state: RwSignal::new(FormState::default()),
            local: Memo::new(move |_| validate()),
            controls: StoredValue::new(Vec::new()),
            reveal: StoredValue::new(None),
        };
        provide_context(form);
        form
    }
    pub(crate) fn on_reveal(self, callback: Callback<String>) {
        self.reveal.set_value(Some(callback));
    }
    pub(crate) fn check(self) -> bool {
        self.check_report(&self.local.get_untracked())
    }
    /// Validate a navigation destination, revealing errors outside the currently visible step.
    pub(crate) fn check_report(self, report: &Report) -> bool {
        self.state.update(|state| state.submit(report.issues()));
        if let Some(issue) = report.issues().first() {
            self.focus_field(issue.field.clone());
            false
        } else {
            true
        }
    }
    pub(crate) fn edited(self, field: &str) {
        self.state.update(|state| state.edit(field));
    }
    pub(crate) fn ticket(self) -> u64 {
        self.state.with_untracked(FormState::ticket)
    }
    pub(crate) fn server(self, ticket: u64, error: &ApiError) {
        if let Some(report) = &error.validation
            && self
                .state
                .try_update(|s| s.receive(ticket, report.clone()))
                .unwrap_or(false)
        {
            self.focus();
        }
    }
    pub(crate) fn focus(self) {
        let first = self.state.with_untracked(|s| {
            self.local
                .with_untracked(|r| s.issues(r.issues()).into_iter().next())
        });
        let Some(issue) = first else {
            return;
        };
        self.focus_field(issue.field);
    }
    fn focus_field(self, field: String) {
        if let Some(reveal) = self.reveal.get_value() {
            reveal.run(field.clone());
        }
        leptos::leptos_dom::helpers::request_animation_frame(move || {
            let _ = self.controls.try_with_value(|controls| {
                if let Some((_, _, focus)) = controls
                    .iter()
                    .find(|(registered, _, _)| *registered == field)
                {
                    focus.run(());
                }
            });
        });
    }
    pub(crate) fn error(self, field: &str) -> Option<Code> {
        self.state
            .with(|s| self.local.with(|r| s.field(r.issues(), field)))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ValidationBinding {
    field: StoredValue<String>,
    id: StoredValue<String>,
    form: Option<FormValidation>,
}
impl ValidationBinding {
    pub(crate) fn new(field: String) -> Self {
        Self {
            form: (!field.is_empty())
                .then(use_context::<FormValidation>)
                .flatten(),
            field: StoredValue::new(field),
            id: StoredValue::new(format!(
                "field-error-{}",
                CONTROL_ID.fetch_add(1, Ordering::Relaxed)
            )),
        }
    }
    pub(crate) fn register(self, focus: Callback<()>) {
        if let Some(form) = self.form {
            form.controls.update_value(|controls| {
                controls.push((self.field.get_value(), self.id.get_value(), focus))
            });
            let control_id = self.id.get_value();
            on_cleanup(move || {
                let _ = form
                    .controls
                    .try_update_value(|controls| controls.retain(|(_, id, _)| *id != control_id));
            });
        }
    }
    pub(crate) fn edited(self) {
        if let Some(form) = self.form {
            form.state
                .update(|state| state.edit(&self.field.get_value()));
        }
    }
    pub(crate) fn touched(self) {
        if let Some(form) = self.form {
            form.state
                .update(|state| state.touch(&self.field.get_value()));
        }
    }
    pub(crate) fn invalid(self) -> bool {
        self.form
            .and_then(|form| form.error(&self.field.get_value()))
            .is_some()
    }
    pub(crate) fn described_by(self) -> Option<String> {
        self.invalid().then(|| self.id.get_value())
    }
    pub(crate) fn view(self) -> impl IntoView {
        let i18n = ruphish_ui::localization::use_i18n();
        view! {
            <Show when=move || self.invalid()>
                <span class="field-error" id=self.id.get_value()>
                    {move || {
                        self.form
                            .and_then(|form| form.error(&self.field.get_value()))
                            .map(|code| ruphish_ui::localization::validation_label(i18n, code))
                    }}
                </span>
            </Show>
        }
    }
}

#[component]
pub(crate) fn ValidationSummary(form: FormValidation) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let issues = Memo::new(move |_| {
        form.state
            .with(|s| form.local.with(|r| s.issues(r.issues())))
    });
    view! {
        <div class="validation-summary" role="alert" aria-atomic="true">
            <Show when=move || !issues.get().is_empty()>
                <p>{move || leptos_i18n::t_string!(i18n, validation.summary)}</p>
                <ul>
                    {move || {
                        issues
                            .get()
                            .into_iter()
                            .filter(|issue| {
                                !form
                                    .controls
                                    .with_value(|controls| {
                                        controls.iter().any(|(field, _, _)| *field == issue.field)
                                    })
                            })
                            .map(|issue| {
                                view! {
                                    <li>
                                        {ruphish_ui::localization::validation_label(
                                            i18n,
                                            issue.code,
                                        )}
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
