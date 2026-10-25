//! Campaign request contracts with additive stable selection and capture overrides.
use crate::legacy::Timestamp;
use serde::{Deserialize, Serialize};
/// Legacy nested resource selection; IDs remain ignored for legacy name lookup.
#[derive(Clone, Default, Serialize)]
pub struct NamedResource {
    /// Legacy lookup name.
    pub name: Option<String>,
    /// Parsed for legacy type compatibility, not used as a new selection mechanism.
    pub id: Option<i64>,
}
/// Explicit new-UI selection, separate from legacy nested resource IDs.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct ReviewedResource {
    /// Owner-scoped persistent resource ID.
    pub id: i64,
    /// Revision shown during review, if already reviewed.
    pub revision: Option<String>,
}
/// IDs/revisions supplied only by clients opting into exact resource selection.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Selections {
    /// Ordered audience collections.
    pub groups: Vec<ReviewedResource>,
    /// Email content.
    pub template: ReviewedResource,
    /// Landing page.
    pub page: ReviewedResource,
    /// Sending profile.
    pub smtp: ReviewedResource,
}
/// Compatibility inputs and optional redirect defaults.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Policy {
    /// Deprecated input, ignored; submissions are always tracked.
    pub capture_credentials: Option<bool>,
    /// Deprecated input, ignored; password presence is always tracked.
    pub capture_passwords: Option<bool>,
    /// Post-submission redirect override.
    pub redirect_url: Option<String>,
    /// Selected owned learning page; custom redirect must be empty when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learning_page_id: Option<i64>,
}
/// Local time entry resolved authoritatively on the server.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalTime {
    /// Civil date/time without an offset.
    pub local: String,
    /// IANA time-zone name.
    pub time_zone: String,
}
/// Compatible legacy creation fields with opt-in UI extensions.
#[derive(Clone, Default, Serialize)]
pub struct Create {
    /// Campaign name.
    pub name: Option<String>,
    /// Legacy ordered group names.
    pub groups: Option<Vec<NamedResource>>,
    /// Legacy email template name.
    pub template: Option<NamedResource>,
    /// Legacy page name.
    pub page: Option<NamedResource>,
    /// Legacy sending-profile name.
    pub smtp: Option<NamedResource>,
    /// Public URL template.
    pub url: Option<String>,
    /// UTC or offset timestamp; zero/null means now.
    pub launch_date: Option<Timestamp>,
    /// Optional initial distribution end.
    pub send_by_date: Option<Timestamp>,
    /// Explicit stable resource selection for the UI.
    pub selection: Option<Selections>,
    /// Optional capture/redirect overrides.
    pub policy: Option<Policy>,
    /// Optional named-zone launch input.
    pub launch_local: Option<LocalTime>,
    /// Optional named-zone distribution end.
    pub send_by_local: Option<LocalTime>,
}

impl<'de> Deserialize<'de> for NamedResource {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Fields;
        impl<'de> serde::de::Visitor<'de> for Fields {
            type Value = NamedResource;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a resource object")
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(NamedResource::default())
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut m: M,
            ) -> Result<Self::Value, M::Error> {
                let mut result = NamedResource::default();
                while let Some(key) = m.next_key::<String>()? {
                    match key.to_ascii_lowercase().as_str() {
                        "name" => {
                            if let Some(v) = m.next_value::<Option<String>>()? {
                                result.name = Some(v);
                            }
                        }
                        "id" => {
                            if let Some(v) = m.next_value::<Option<i64>>()? {
                                result.id = Some(v);
                            }
                        }
                        _ => {
                            m.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(result)
            }
        }
        d.deserialize_any(Fields)
    }
}
impl<'de> Deserialize<'de> for Create {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Fields;
        impl<'de> serde::de::Visitor<'de> for Fields {
            type Value = Create;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a campaign object")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut m: M,
            ) -> Result<Self::Value, M::Error> {
                let mut result = Create::default();
                while let Some(key) = m.next_key::<String>()? {
                    macro_rules! scalar {
                        ($field:ident,$ty:ty) => {
                            if let Some(value) = m.next_value::<Option<$ty>>()? {
                                result.$field = Some(value);
                            }
                        };
                    }
                    macro_rules! resource {
                        ($field:ident) => {
                            if let Some(value) = m.next_value::<Option<NamedResource>>()? {
                                let current =
                                    result.$field.get_or_insert_with(NamedResource::default);
                                if value.name.is_some() {
                                    current.name = value.name;
                                }
                                if value.id.is_some() {
                                    current.id = value.id;
                                }
                            }
                        };
                    }
                    match key.to_ascii_lowercase().as_str() {
                        "name" => {
                            scalar!(name, String);
                        }
                        "url" => {
                            scalar!(url, String);
                        }
                        "launch_date" => {
                            scalar!(launch_date, Timestamp);
                        }
                        "send_by_date" => {
                            scalar!(send_by_date, Timestamp);
                        }
                        "template" => {
                            resource!(template);
                        }
                        "page" => {
                            resource!(page);
                        }
                        "smtp" => {
                            resource!(smtp);
                        }
                        "groups" => result.groups = m.next_value()?,
                        "selection" => result.selection = m.next_value()?,
                        "policy" => result.policy = m.next_value()?,
                        "launch_local" => result.launch_local = m.next_value()?,
                        "send_by_local" => result.send_by_local = m.next_value()?,
                        _ => {
                            m.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(result)
            }
        }
        d.deserialize_map(Fields)
    }
}
