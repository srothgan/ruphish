//! Server-schema bounds checked before assignment can silently shorten VARCHAR.
use ruphish_application::resources::{Content, Error, Resource};

/// SQL character limits count Unicode scalar values, never UTF-8 bytes.
pub(crate) fn bounded(value: &str, limit: usize) -> Result<(), Error> {
    if value.chars().nth(limit).is_some() {
        return Err(Error::Invalid(
            "storage_text_invalid: text exceeds the supported field length".into(),
        ));
    }
    Ok(())
}

pub(crate) fn resource(value: &Resource) -> Result<(), Error> {
    bounded(&value.name, 255)?;
    match &value.content {
        Content::Group(targets) => {
            for target in targets {
                for field in [
                    &target.email,
                    &target.first_name,
                    &target.last_name,
                    &target.position,
                ] {
                    bounded(field, 255)?;
                }
            }
        }
        Content::Template(template) => {
            bounded(&template.subject, 255)?;
            bounded(&template.envelope_sender, 255)?;
            for attachment in &template.attachments {
                bounded(&attachment.name, 255)?;
                bounded(&attachment.mime_type, 255)?;
            }
        }
        Content::Profile(profile) => {
            for field in [
                &profile.interface_type,
                &profile.host,
                &profile.username,
                &profile.from_address,
            ] {
                bounded(field, 255)?;
            }
            for header in &profile.headers {
                bounded(&header.key, 255)?;
            }
        }
        Content::Page(_) => {}
    }
    Ok(())
}

pub(crate) fn preview(value: &ruphish_application::delivery::MessageInput) -> Result<(), Error> {
    for field in [
        &value.recipient.email,
        &value.recipient.first_name,
        &value.recipient.last_name,
        &value.recipient.position,
        &value.rid,
        if value.template.envelope_sender.is_empty() {
            &value.profile.from_address
        } else {
            &value.template.envelope_sender
        },
    ] {
        bounded(field, 255)?;
    }
    Ok(())
}
