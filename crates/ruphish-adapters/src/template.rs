//! Go text-template execution for existing stored content; no automatic HTML escaping.

use gtmpl_ng::{Template, Value};

/// Explicit rendering budgets; callers may choose smaller limits for validation.
pub use gtmpl_ng::Limits as RenderLimits;

/// The exported fields available to saved campaign templates.
#[derive(Debug, Default, Clone)]
pub struct Context {
    /// Sender display name or address.
    pub from: String,
    /// Recipient-specific campaign URL.
    pub url: String,
    /// Existing tracking image markup.
    pub tracker: String,
    /// Tracking URL.
    pub tracking_url: String,
    /// Recipient identifier.
    pub rid: String,
    /// Base URL without path/query.
    pub base_url: String,
    /// Recipient address.
    pub email: String,
    /// Recipient first name.
    pub first_name: String,
    /// Recipient last name.
    pub last_name: String,
    /// Recipient position.
    pub position: String,
}

impl Context {
    fn value(&self) -> Value {
        use ruphish_domain::template::Field;
        Value::Object(
            Field::ALL
                .iter()
                .map(|field| {
                    let value = match field {
                        Field::FirstName => &self.first_name,
                        Field::LastName => &self.last_name,
                        Field::Email => &self.email,
                        Field::Position => &self.position,
                        Field::From => &self.from,
                        Field::Url => &self.url,
                        Field::Rid => &self.rid,
                        Field::BaseUrl => &self.base_url,
                        Field::TrackingUrl => &self.tracking_url,
                        Field::Tracker => &self.tracker,
                    };
                    (field.name().into(), value.clone().into())
                })
                .collect(),
        )
    }
}

/// Distinguish invalid syntax from execution failures without freezing engine diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Parsing failed before any output was produced.
    Parse,
    /// Execution failed, possibly after producing partial output.
    Execution,
}

/// An execution failure and the partial output retained by Go's implementation.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct RenderError {
    /// Whether parsing or execution failed.
    pub kind: ErrorKind,
    /// Output preceding the failure.
    pub output: String,
    /// Engine diagnostic; not an API serialization contract.
    pub message: String,
}

/// Render Go templates, preserving text-template escaping behavior.
pub fn render(source: &str, context: &Context) -> Result<String, RenderError> {
    render_with_limits(source, context, RenderLimits::default())
}

/// Render with explicit allocation/iteration budgets without global mutable policy.
pub fn render_with_limits(
    source: &str,
    context: &Context,
    limits: RenderLimits,
) -> Result<String, RenderError> {
    let mut template = Template::with_name("template");
    template.limits = limits;
    template.parse(source).map_err(|error| RenderError {
        kind: ErrorKind::Parse,
        output: String::new(),
        message: error.to_string(),
    })?;
    let mut output = Vec::new();
    let result = template.execute(&mut output, &gtmpl_ng::Context::from(context.value()));
    let output = String::from_utf8(output).map_err(|error| RenderError {
        kind: ErrorKind::Execution,
        output: String::new(),
        message: format!("Template output is not valid UTF-8: {error}"),
    })?;
    match result {
        Ok(()) => Ok(output),
        Err(error) => Err(RenderError {
            kind: ErrorKind::Execution,
            output,
            message: error.to_string(),
        }),
    }
}
