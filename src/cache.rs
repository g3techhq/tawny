use dioxus::prelude::*;
use serde::{Serialize, de::DeserializeOwned};

/// A small cross-platform cache hook backed by the host WebView's persistent
/// local storage. Dioxus uses a WebView on web, desktop, Android, and iOS, so
/// this keeps the first local-first slice portable without embedding a second
/// database engine in every client binary.
pub fn use_persistent_signal<T>(key: &'static str, init: impl FnOnce() -> T) -> Signal<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + PartialEq + 'static,
{
    #[cfg(feature = "server")]
    {
        let _ = key;
        return use_signal(init);
    }

    #[cfg(not(feature = "server"))]
    {
        let mut value = use_signal(init);
        let mut hydrated = use_signal(|| false);
        let key_literal = serde_json::to_string(key).expect("cache key is serializable");

        use_effect(move || {
            if hydrated() {
                return;
            }
            let key_literal = key_literal.clone();
            spawn(async move {
                let script =
                    format!("dioxus.send(window.localStorage.getItem({key_literal}) ?? '');");
                let mut eval = document::eval(&script);
                if let Ok(raw) = eval.recv::<String>().await {
                    if !raw.is_empty() {
                        if let Ok(cached) = serde_json::from_str::<T>(&raw) {
                            value.set(cached);
                        }
                    }
                }
                hydrated.set(true);
            });
        });

        use_effect(move || {
            let snapshot = value();
            if !hydrated() {
                return;
            }
            let Ok(serialized) = serde_json::to_string(&snapshot) else {
                return;
            };
            let serialized_literal = serde_json::to_string(&serialized)
                .expect("serialized cache snapshot is a valid JS literal");
            let key_literal = serde_json::to_string(key).expect("cache key is serializable");
            spawn(async move {
                let script = format!(
                    "window.localStorage.setItem({key_literal}, {serialized_literal}); dioxus.send(true);"
                );
                let mut eval = document::eval(&script);
                let _ = eval.recv::<bool>().await;
            });
        });

        value
    }
}
