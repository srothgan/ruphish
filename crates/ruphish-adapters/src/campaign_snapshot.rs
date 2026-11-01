//! Immutable campaign content. Transport settings and credentials are deliberately absent.
use ruphish_application::resources::{Content, Error, Header, Page, Profile, Resource, Template};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct Frozen<T> {
    #[serde(default)]
    pub attribution: ruphish_domain::attribution::Attribution,
    pub id: i64,
    pub name: String,
    pub revision: i64,
    pub modified_date: String,
    pub content: T,
}
impl<T> Frozen<T> {
    fn new(resource: &Resource, content: T) -> Self {
        Self {
            attribution: resource.attribution.clone(),
            id: resource.id,
            name: resource.name.clone(),
            revision: resource.revision,
            modified_date: resource.modified_date.clone(),
            content,
        }
    }
    fn resource(&self, content: Content) -> Resource {
        Resource {
            attribution: self.attribution.clone(),
            revision: self.revision,
            id: self.id,
            name: self.name.clone(),
            modified_date: self.modified_date.clone(),
            content,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Sender {
    pub from_address: String,
    pub headers: Vec<Header>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Snapshot {
    pub template: Frozen<Template>,
    pub page: Frozen<Page>,
    pub sender: Frozen<Sender>,
    pub learning_html: Option<String>,
}
impl Snapshot {
    pub fn capture(
        template: &Resource,
        page: &Resource,
        profile: &Resource,
        learning: Option<&Resource>,
    ) -> Result<Self, Error> {
        let (Content::Template(t), Content::Page(p), Content::Profile(s)) =
            (&template.content, &page.content, &profile.content)
        else {
            return Err(Error::Internal);
        };
        let learning_html = learning
            .map(|r| match &r.content {
                Content::LearningPage(p) => Ok(p.html.clone()),
                _ => Err(Error::Internal),
            })
            .transpose()?;
        Ok(Self {
            template: Frozen::new(template, t.clone()),
            page: Frozen::new(page, p.clone()),
            sender: Frozen::new(
                profile,
                Sender {
                    from_address: s.from_address.clone(),
                    headers: s.headers.clone(),
                },
            ),
            learning_html,
        })
    }
    pub fn encode(&self) -> Result<String, Error> {
        serde_json::to_string(self).map_err(|_| Error::Internal)
    }
    pub fn decode(value: &str) -> Result<Self, Error> {
        serde_json::from_str(value).map_err(|_| Error::Internal)
    }
    pub fn template(&self) -> Resource {
        self.template
            .resource(Content::Template(self.template.content.clone()))
    }
    pub fn page(&self) -> Resource {
        self.page.resource(Content::Page(self.page.content.clone()))
    }
    pub fn sender(&self) -> Resource {
        self.sender
            .resource(Content::Profile(self.apply_sender(Profile::default())))
    }
    pub fn apply_sender(&self, mut transport: Profile) -> Profile {
        transport.from_address = self.sender.content.from_address.clone();
        transport.headers = self.sender.content.headers.clone();
        transport
    }
}
