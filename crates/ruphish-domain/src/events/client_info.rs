//! Coarse request categories. No raw headers, versions, models, or network identities.
use serde::{Deserialize, Serialize};

macro_rules! categories {
    ($name:ident, $($variant:ident => $key:literal),+ $(,)?) => {
        /// Closed vocabulary; arbitrary request strings cannot cross this boundary.
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name {
            /// Unrecognized or unavailable category.
            #[default]
            #[serde(rename = "unknown")]
            Unknown,
            $(#[doc = $key] #[serde(rename = $key)] $variant,)+
        }
        impl $name {
            /// Stable storage and export identifier.
            pub const fn key(self) -> &'static str {
                match self { Self::Unknown => "unknown", $(Self::$variant => $key,)+ }
            }
        }
    };
}
categories!(Browser, Chrome => "chrome", Edge => "edge", Firefox => "firefox", Safari => "safari", Opera => "opera", SamsungInternet => "samsung_internet", InternetExplorer => "internet_explorer");
categories!(OperatingSystem, Windows => "windows", MacOs => "macos", Linux => "linux", Android => "android", Ios => "ios", ChromeOs => "chromeos");
categories!(Device, Desktop => "desktop", Mobile => "mobile", Tablet => "tablet");

/// Categories observed on one request, not a verified participant identity or device.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Info {
    /// Browser family without a version.
    pub browser: Browser,
    /// Operating-system family without a version.
    pub os: OperatingSystem,
    /// Coarse device class without a model.
    pub device: Device,
}
impl Info {
    /// Classify a bounded header conservatively. Missing, malformed and known automated
    /// clients are unknown; reduced and spoofed headers cannot establish actual identity.
    pub fn from_user_agent(header: Option<&str>) -> Self {
        let Some(header) = header.filter(|s| {
            !s.is_empty() && s.len() <= 2048 && s.bytes().all(|b| (32..=126).contains(&b))
        }) else {
            return Self::default();
        };
        let ua = header.to_ascii_lowercase();
        if ["bot", "crawler", "spider", "headless", "curl/", "wget/"]
            .iter()
            .any(|token| ua.contains(token))
        {
            return Self::default();
        }
        let product = |name: &str| {
            ua.match_indices(name).any(|(i, _)| {
                (i == 0 || matches!(ua.as_bytes()[i - 1], b' ' | b'(' | b';'))
                    && ua
                        .as_bytes()
                        .get(i + name.len())
                        .is_some_and(u8::is_ascii_digit)
            })
        };
        let browser =
            if product("edg/") || product("edga/") || product("edgios/") || product("edge/") {
                Browser::Edge
            } else if product("opr/") || product("opera/") || product("opios/") {
                Browser::Opera
            } else if product("samsungbrowser/") {
                Browser::SamsungInternet
            } else if product("firefox/") || product("fxios/") {
                Browser::Firefox
            } else if product("chrome/") || product("crios/") {
                Browser::Chrome
            } else if product("safari/") && product("version/") {
                Browser::Safari
            } else if product("msie ") || product("trident/") {
                Browser::InternetExplorer
            } else {
                Browser::Unknown
            };
        let os = if ua.contains("iphone") || ua.contains("ipad") || ua.contains("ipod") {
            OperatingSystem::Ios
        } else if ua.contains("android") {
            OperatingSystem::Android
        } else if ua.contains("windows nt ") {
            OperatingSystem::Windows
        } else if ua.contains("cros ") {
            OperatingSystem::ChromeOs
        } else if ua.contains("macintosh") || ua.contains("mac os x") {
            OperatingSystem::MacOs
        } else if ua.contains("linux") {
            OperatingSystem::Linux
        } else {
            OperatingSystem::Unknown
        };
        let device =
            if ua.contains("ipad") || (os == OperatingSystem::Android && !ua.contains("mobile")) {
                Device::Tablet
            } else if ua.contains("iphone")
                || ua.contains("ipod")
                || (os == OperatingSystem::Android && ua.contains("mobile"))
            {
                Device::Mobile
            } else if os == OperatingSystem::MacOs && ua.contains("mobile/") {
                Device::Unknown
            } else if matches!(
                os,
                OperatingSystem::Windows
                    | OperatingSystem::MacOs
                    | OperatingSystem::Linux
                    | OperatingSystem::ChromeOs
            ) {
                Device::Desktop
            } else {
                Device::Unknown
            };
        Self {
            browser,
            os,
            device,
        }
    }
}
