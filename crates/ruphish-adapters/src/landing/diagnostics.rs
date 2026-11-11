//! Advisory diagnostics on the same processed document that participants receive.
use ruphish_application::resources::{Error, PageWarning};
use scraper::{ElementRef, Html, Selector};

fn disabled(control: ElementRef<'_>) -> bool {
    control.value().attr("disabled").is_some()
        || control
            .ancestors()
            .filter_map(ElementRef::wrap)
            .any(|parent| {
                if parent.value().name() != "fieldset" || parent.value().attr("disabled").is_none()
                {
                    return false;
                }
                // HTML exempts descendants of the disabled fieldset's first legend.
                let legend = parent
                    .children()
                    .filter_map(ElementRef::wrap)
                    .find(|child| child.value().name() == "legend");
                !legend
                    .is_some_and(|legend| control.ancestors().any(|node| node.id() == legend.id()))
            })
}

fn owner<'a>(
    control: ElementRef<'a>,
    ids: &std::collections::HashMap<&'a str, ElementRef<'a>>,
) -> Option<ElementRef<'a>> {
    if let Some(id) = control.value().attr("form") {
        return ids
            .get(id)
            .copied()
            .filter(|element| element.value().name() == "form");
    }
    control
        .ancestors()
        .filter_map(ElementRef::wrap)
        .find(|parent| parent.value().name() == "form")
}

pub(super) fn inspect(source: &str, processed: &str) -> Result<Vec<PageWarning>, Error> {
    let selector = Selector::parse("*").map_err(|_| Error::Internal)?;
    let original = Html::parse_document(source);
    let document = Html::parse_document(processed);
    let elements: Vec<_> = document.select(&selector).collect();
    let mut warnings = Vec::new();
    let mut warn = |code: &'static str, control: Option<usize>| {
        if warnings.len() < 100 {
            warnings.push(PageWarning { code, control });
        }
    };
    if original.select(&selector).any(|node| {
        node.value().name() == "script"
            || node.value().attrs().any(|(name, _)| name.starts_with("on"))
    }) {
        warn("authored_execution_removed", None);
    }
    if original
        .errors
        .iter()
        .any(|error| !error.to_ascii_lowercase().contains("doctype"))
    {
        warn("html_repaired", None);
    }
    let mut ids = std::collections::HashMap::new();
    let mut duplicate = false;
    for element in &elements {
        if let Some(id) = element.value().id() {
            match ids.entry(id) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(*element);
                }
                std::collections::hash_map::Entry::Occupied(_) => duplicate = true,
            }
        }
    }
    if duplicate {
        warn("duplicate_control_id", None);
    }
    for (index, control) in elements
        .iter()
        .copied()
        .filter(|element| element.value().name() == "input")
        .enumerate()
    {
        if !control
            .value()
            .attr("type")
            .is_some_and(|kind| kind.eq_ignore_ascii_case("password"))
        {
            continue;
        }
        if disabled(control) {
            warn("password_disabled", Some(index + 1));
        }
        if control.value().attr("name").is_none_or(str::is_empty) {
            warn("password_unnamed", Some(index + 1));
        }
        if owner(control, &ids).is_none() {
            warn("password_without_form", Some(index + 1));
        }
    }
    let mut forms: std::collections::HashMap<_, (bool, usize)> = elements
        .iter()
        .filter(|element| element.value().name() == "form")
        .map(|element| (element.id(), (false, 0)))
        .collect();
    for control in elements
        .iter()
        .copied()
        .filter(|control| matches!(control.value().name(), "input" | "button"))
    {
        if disabled(control) {
            continue;
        }
        let Some(form) = owner(control, &ids) else {
            continue;
        };
        let Some((button, implicit)) = forms.get_mut(&form.id()) else {
            continue;
        };
        let kind = control
            .value()
            .attr("type")
            .unwrap_or("text")
            .to_ascii_lowercase();
        *button |= (control.value().name() == "button"
            && !matches!(kind.as_str(), "button" | "reset"))
            || (control.value().name() == "input" && matches!(kind.as_str(), "submit" | "image"));
        if control.value().name() == "input"
            && matches!(
                kind.as_str(),
                "text"
                    | "search"
                    | "url"
                    | "tel"
                    | "email"
                    | "password"
                    | "date"
                    | "month"
                    | "week"
                    | "time"
                    | "datetime-local"
                    | "number"
            )
        {
            *implicit += 1;
        }
    }
    for (button, implicit) in forms.into_values() {
        if !button && implicit != 1 {
            warn("form_without_submit", None);
        }
    }
    Ok(warnings)
}
