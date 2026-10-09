use dioxus::prelude::*;

/// False for a page's first frame, true once that frame has been painted.
///
/// A route transition holds the outgoing page on screen until the incoming one
/// has rendered, so a page that works through the whole library before its
/// first frame reads as the app freezing on the page being left. Pages with
/// that kind of work draw their frame and placeholders first, and wait on this
/// before doing it.
pub fn use_after_first_paint() -> ReadSignal<bool> {
    let mut painted = use_signal(|| false);
    use_effect(move || {
        spawn(async move {
            // A frame, then a task, so the work lands after that frame's paint.
            // The timer is a ceiling for a webview that has stopped drawing,
            // where frame callbacks never come.
            let mut eval = document::eval(
                "let sent = false;
                const done = () => { if (!sent) { sent = true; dioxus.send(true); } };
                requestAnimationFrame(() => setTimeout(done, 0));
                setTimeout(done, 200);",
            );
            let _ = eval.recv::<bool>().await;
            painted.set(true);
        });
    });
    painted.into()
}

/// False until the route transition that brought this page in has finished.
///
/// Anything that re-renders the page while it is sliding in - a request
/// landing, a library write - runs on the main thread the animation needs, and
/// shows as the slide stuttering. Work that can wait a moment waits on this.
/// The library owns the wait, so every g3 app settles the same way.
pub fn use_after_route_transition() -> ReadSignal<bool> {
    g3_route_transitions::use_route_transition_settled()
}
