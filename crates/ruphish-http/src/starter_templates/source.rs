//! Bundled catalog metadata and ordinary resource creation payloads.
use ruphish_contracts::starter_templates::*;
use serde_json::json;

pub(super) struct Source {
    pub(super) id: &'static str,
    pub(super) name: &'static str,
    pub(super) brand: &'static str,
    pub(super) html: &'static str,
    pub(super) kind: Kind,
    pub(super) subject: &'static str,
    pub(super) text: &'static str,
    pub(super) logo: Option<(&'static str, u16, &'static str)>,
}

impl Source {
    pub(super) fn summary(&self) -> Summary {
        Summary {
            id: self.id.into(),
            name: self.name.into(),
            brand: self.brand.into(),
            kind: self.kind,
            prefill: self.kind == Kind::LandingPage,
            company_logo: self.logo.filter(|_| self.kind != Kind::Email).map(
                |(selector, height, context)| CompanyLogo {
                    selector: selector.into(),
                    height,
                    context: context.into(),
                },
            ),
        }
    }

    pub(super) fn entry(&self) -> Entry {
        Entry {
            summary: self.summary(),
            resource: match self.kind {
                Kind::Email => json!({
                    "name": self.name, "html": self.html,
                    "subject": self.subject.trim_end(), "text": self.text, "attachments": [],
                }),
                Kind::LandingPage => {
                    json!({"name": self.name, "html": self.html, "redirect_url": ""})
                }
                Kind::LearningPage => json!({"name": self.name, "html": self.html}),
            },
        }
    }
}
