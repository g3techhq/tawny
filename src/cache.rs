use dioxus::prelude::*;
use serde::{Serialize, de::DeserializeOwned};

/// Resolves when a cache write can go out without being felt.
///
/// The write itself is one eval carrying the whole serialized value, and for
/// the library that is megabytes: sent over the Rust-to-WebView bridge, parsed
/// and stored on the WebView's main thread. Sent the moment anything changed,
/// it landed in the middle of route transitions - expanding the mini player
/// records history, so it did every time - and every five seconds during
/// playback, when progress ticks. So a write waits out a delay (which also
/// coalesces the changes behind it), any running transition, and then idle
/// time. Backgrounding cuts the wait short: that is the last moment a write
/// is sure to land before the app may be killed.
#[cfg(not(feature = "server"))]
const PERSIST_WHEN_IDLE_JS: &str = r#"
const delay = __PERSIST_DELAY_MS__;
// Awaited to the end: a native renderer closes the channel once the script
// returns, and a send after that never reaches Rust.
await new Promise((resolve) => {
    let sent = false;
    const done = () => {
        if (sent) return;
        sent = true;
        document.removeEventListener('visibilitychange', onVisibility);
        resolve();
    };
    const onVisibility = () => { if (document.visibilityState === 'hidden') done(); };
    document.addEventListener('visibilitychange', onVisibility);
    setTimeout(async () => {
        // Timers rather than frames: frames stop ticking in a background tab.
        for (let waited = 0; !sent && waited < 5000 && document.documentElement.dataset.routeTransition; waited += 50) {
            await new Promise((next) => setTimeout(next, 50));
        }
        if (sent) return;
        if (window.requestIdleCallback) window.requestIdleCallback(done, { timeout: 1000 });
        else setTimeout(done, 0);
    }, delay);
});
dioxus.send(true);
"#;

/// A small cross-platform cache hook backed by the host WebView's persistent
/// local storage. Dioxus uses a WebView on web, desktop, Android, and iOS, so
/// this keeps the first local-first slice portable without embedding a second
/// database engine in every client binary.
///
/// Changes reach storage at most once per `persist_after_ms`, and never during
/// a route transition - see [`PERSIST_WHEN_IDLE_JS`]. Size the delay to the
/// value: a large one that changes often wants a long one.
pub fn use_persistent_signal<T>(
    key: &'static str,
    persist_after_ms: u32,
    init: impl FnOnce() -> T,
) -> Signal<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + PartialEq + 'static,
{
    #[cfg(feature = "server")]
    {
        let _ = (key, persist_after_ms);
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

        // Whether a write is already waiting. Later changes ride along with it:
        // it serializes whatever the value is by the time it goes out.
        let scheduled = use_hook(|| std::rc::Rc::new(std::cell::Cell::new(false)));
        use_effect(move || {
            // Read rather than cloned: this only subscribes, and a clone of the
            // library on every change was itself a cost.
            let _ = value.read();
            if !hydrated() || scheduled.replace(true) {
                return;
            }
            let scheduled = scheduled.clone();
            spawn(async move {
                let gate = PERSIST_WHEN_IDLE_JS
                    .replace("__PERSIST_DELAY_MS__", &persist_after_ms.to_string());
                let _ = document::eval(&gate).recv::<bool>().await;
                // Released before serializing, so a change that lands while
                // this write is out schedules the next one instead of being
                // folded into a snapshot already taken.
                scheduled.set(false);
                let Ok(serialized) = serde_json::to_string(&*value.peek()) else {
                    return;
                };
                let serialized_literal = serde_json::to_string(&serialized)
                    .expect("serialized cache snapshot is a valid JS literal");
                let key_literal = serde_json::to_string(key).expect("cache key is serializable");
                let script = format!(
                    "window.localStorage.setItem({key_literal}, {serialized_literal}); dioxus.send(true);"
                );
                let _ = document::eval(&script).recv::<bool>().await;
            });
        });

        value
    }
}
