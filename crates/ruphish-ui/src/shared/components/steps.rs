//! Button-based navigation through a sequential editing workflow.
use super::ActionButton;
use leptos::prelude::*;

#[component]
pub(crate) fn StepNavigation(
    #[prop(into)] label: Signal<String>,
    labels: Vec<Signal<String>>,
    #[prop(into)] current: Signal<usize>,
    #[prop(into)] disabled: Signal<bool>,
    #[prop(into)] disabled_steps: Signal<Vec<usize>>,
    on_select: Callback<usize>,
) -> impl IntoView {
    view! {
        <nav aria-label=move || label.get()>
            <ol class="step-navigation">
                {labels
                    .into_iter()
                    .enumerate()
                    .map(|(index, label)| {
                        view! {
                            <li>
                                <ActionButton
                                    current=Signal::derive(move || {
                                        if current.get() == index {
                                            "step".into()
                                        } else {
                                            String::new()
                                        }
                                    })
                                    disabled=Signal::derive(move || {
                                        disabled.get()
                                            || disabled_steps.with(|steps| steps.contains(&index))
                                    })
                                    on_click=move |_| on_select.run(index)
                                >
                                    <span class="step-navigation-number" aria-hidden="true">
                                        {index + 1}
                                    </span>
                                    <span>{move || label.get()}</span>
                                </ActionButton>
                            </li>
                        }
                    })
                    .collect_view()}
            </ol>
        </nav>
    }
}
