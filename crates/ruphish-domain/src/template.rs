//! Browser-safe vocabulary of supported authored template variables.
macro_rules! fields {
    ($($(#[$doc:meta])* $variant:ident => $name:literal),* $(,)?) => {
        /// A supported field, independent of the template execution library.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Field { $($(#[$doc])* $variant),* }
        impl Field {
            /// Every supported field, in editor display order.
            pub const ALL: &[Self] = &[$(Self::$variant),*];
            /// Exact case-sensitive name in saved Go templates.
            pub const fn name(self) -> &'static str {
                match self { $(Self::$variant => $name),* }
            }
        }
    };
}
fields! {
    /// Recipient given name.
    FirstName => "FirstName",
    /// Recipient family name.
    LastName => "LastName",
    /// Recipient mailbox.
    Email => "Email",
    /// Recipient position.
    Position => "Position",
    /// Visible sender.
    From => "From",
    /// Personalized campaign URL.
    Url => "URL",
    /// Public recipient identifier.
    Rid => "RId",
    /// Campaign base URL.
    BaseUrl => "BaseURL",
    /// Tracking pixel URL.
    TrackingUrl => "TrackingURL",
    /// Tracking image markup.
    Tracker => "Tracker",
}
