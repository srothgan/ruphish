//! Single authority for browser `localStorage`, shared by every persisted UI setting.
//!
//! All access funnels through here so keys stay namespaced and consistent, and so a
//! browser without `localStorage` (private modes, storage disabled, quota errors)
//! degrades to the caller's default instead of failing. Values are stored as JSON, which
//! keeps the helpers usable for any serializable setting rather than the sidebar alone.
use leptos::prelude::*;
use serde::{Serialize, de::DeserializeOwned};

/// Prefix applied to every key so Ruphish settings never collide with other origin state.
const NAMESPACE: &str = "ruphish:";

fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|window| window.local_storage().ok().flatten())
}

fn namespaced(key: &str) -> String {
    format!("{NAMESPACE}{key}")
}

/// Read and decode a stored setting, returning `None` when it is absent or unreadable.
pub fn get<T: DeserializeOwned>(key: &str) -> Option<T> {
    let raw = storage()?.get_item(&namespaced(key)).ok().flatten()?;
    serde_json::from_str(&raw).ok()
}

/// Encode and persist a setting. The write is silently skipped when storage is unavailable.
pub fn set<T: Serialize>(key: &str, value: &T) {
    if let (Some(storage), Ok(raw)) = (storage(), serde_json::to_string(value)) {
        let _ = storage.set_item(&namespaced(key), &raw);
    }
}

/// Forget a setting, so the next read falls back to the caller's default.
pub fn remove(key: &str) {
    if let Some(storage) = storage() {
        let _ = storage.remove_item(&namespaced(key));
    }
}

/// A reactive setting backed by `localStorage`, for callers that want a live signal.
///
/// The signal starts from the stored value, falling back to `default` when the key is
/// absent or unreadable, and writes every later change back to storage. The initial value
/// is never written, so a setting the user never touches leaves no stored entry behind.
pub fn persistent<T>(key: &'static str, default: T) -> RwSignal<T>
where
    T: Clone + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    let signal = RwSignal::new(get::<T>(key).unwrap_or(default));
    Effect::new(move |run: Option<()>| {
        let value = signal.get();
        if run.is_some() {
            set(key, &value);
        }
    });
    signal
}
