//! Who the viewer is, and which backend they are talking to.
//!
//! The flow a first launch takes:
//!
//! 1. No backend has been chosen, so [`AccountGate`] shows the setup screen
//!    instead of the app. Nothing else can work until this is answered - there
//!    is no library to show and no server to ask.
//! 2. With a backend, the gate mints a guest account and stores its token. No
//!    sign-in wall: the app is usable immediately, and the account exists so
//!    that everything saved from this moment has an owner.
//! 3. Later, from settings, that guest can be promoted to a real account
//!    (keeping its library) or abandoned by signing in to another one.

use dioxus::prelude::*;
use g3_ui::{Button, ButtonStyle, Card, Field, StatusColor};

use crate::{
    api::{register_account, sign_in_to_account, sign_out_of_account},
    config,
    models::{Credentials, validate_email, validate_password},
    session::{SessionStatus, readable, use_session},
    state::AppState,
};

/// Reload the client so a new backend URL takes effect.
///
/// `set_server_url` keeps only the first value it is given, so there is no way
/// to repoint a running process - see `config`. On the web a reload is a fresh
/// process; a packaged client has to be restarted by hand, which is what the
/// setup screen says.
fn reload_client() {
    spawn(async move {
        let mut eval = document::eval("window.location.reload(); dioxus.send(true);");
        let _ = eval.recv::<bool>().await;
    });
}

/// Chooses between the app and the "connect to a server" screen.
///
/// **The first render has to match what the server rendered.** Reading
/// `has_chosen_backend()` in the signal's initialiser did not: it consults
/// localStorage on the client and is always false on the server, so the moment
/// a backend had been saved the client hydrated a Router over a server-rendered
/// setup screen. Dioxus's interpreter then walked a node table that did not
/// describe the DOM in front of it and died on
/// `Cannot set properties of undefined (setting 'textContent')`.
///
/// So both sides start by rendering the app, and the real answer arrives in an
/// effect, which only ever runs on the client. A configured viewer - almost
/// everyone, almost always - sees no flash; a first launch sees the app for a
/// frame before the setup screen replaces it, which is the cheaper of the two
/// mistakes to make.
#[component]
pub fn AccountGate(children: Element) -> Element {
    let session = use_session();
    let mut backend_chosen = use_signal(|| true);

    use_effect(move || backend_chosen.set(config::has_chosen_backend()));

    if !backend_chosen() {
        return rsx! {
            BackendSetupScreen {
                on_saved: move |_| {
                    backend_chosen.set(true);
                    reload_client();
                },
            }
        };
    }

    match (session.status)() {
        SessionStatus::Failed(message) => rsx! {
            BackendSetupScreen {
                problem: message,
                on_saved: move |_| reload_client(),
            }
        },
        // The app renders while the guest is being minted rather than behind a
        // spinner. Everything on screen at that moment comes from the local
        // cache, and blocking it would make a cold start feel like a network
        // failure.
        _ => rsx! { {children} },
    }
}

#[component]
fn BackendSetupScreen(problem: Option<String>, on_saved: EventHandler<()>) -> Element {
    let url = use_signal(config::backend_url);
    let mut error = use_signal(|| problem.clone().unwrap_or_default());

    rsx! {
        main { class: "page account-setup",
            Card {
                h1 { "Connect to a Tawny server" }
                p { class: "account-setup-lead",
                    "Tawny stores your subscriptions and history on a server you or someone you trust runs. Enter its address to begin."
                }
                Field {
                    label: "Server address".to_string(),
                    value: url,
                    r#type: "url".to_string(),
                    placeholder: "https://tawny.example".to_string(),
                    oninput: move |_| error.set(String::new()),
                }
                if !error().is_empty() {
                    p { class: "account-error", "{error}" }
                }
                Button {
                    style: ButtonStyle::Solid,
                    expand: true,
                    onclick: move |_| {
                        match config::set_backend_url(&url()) {
                            Some(_) => on_saved.call(()),
                            None => error.set(
                                "Enter a full address, including http:// or https://.".into(),
                            ),
                        }
                    },
                    "Connect"
                }
                p { class: "account-setup-note",
                    "Changing this restarts the app. On a packaged build, close and reopen it."
                }
            }
        }
    }
}

/// The account section of the settings page.
#[component]
pub fn AccountSettings() -> Element {
    let app_state = use_context::<AppState>();
    let mut session = use_session();
    let account = (session.account)();

    let email = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    // A guest sees the sign-up form; anyone else sees sign-in. Both post to a
    // different endpoint, so which one is showing has to be explicit.
    let mut signing_in = use_signal(|| false);

    let Some(account) = account else {
        return rsx! {
            Card { p { class: "detail-muted", "Connecting to your server…" } }
        };
    };

    let is_guest = account.is_guest;
    let upgrade = is_guest && !signing_in();

    rsx! {
        Card {
            h2 { "Account" }
            if !account.is_recoverable() {
                p { class: "account-warning",
                    "You are signed in as a guest. This library lives only on this device's session - add an email and password to be able to sign back in, here or anywhere else."
                }
            } else {
                p { class: "detail-muted", "Signed in as {account.label()}" }
            }

            if is_guest || signing_in() {
                Field {
                    label: "Email".to_string(),
                    value: email,
                    r#type: "email".to_string(),
                    autocomplete: "username",
                    oninput: move |_| error.set(String::new()),
                }
                Field {
                    label: "Password".to_string(),
                    value: password,
                    r#type: "password".to_string(),
                    // Tells a password manager to offer a new suggestion when
                    // this is a sign-up and the stored one when it is not.
                    autocomplete: if upgrade { "new-password" } else { "current-password" },
                    oninput: move |_| error.set(String::new()),
                }
                if !error().is_empty() {
                    p { class: "account-error", "{error}" }
                }
                Button {
                    style: ButtonStyle::Solid,
                    expand: true,
                    disabled: busy(),
                    onclick: move |_| {
                        // Validated here first so the field-level rules in
                        // `models` mark the input rather than arriving as
                        // server prose. The server checks them again; this is
                        // for the message, not for trust.
                        if upgrade {
                            if let Err(problem) = validate_email(&email()) {
                                error.set(problem.message().into());
                                return;
                            }
                            if let Err(problem) = validate_password(&password()) {
                                error.set(problem.message().into());
                                return;
                            }
                        }
                        let credentials = Credentials { email: email(), password: password() };
                        busy.set(true);
                        spawn(async move {
                            let result = if upgrade {
                                register_account(credentials).await
                            } else {
                                sign_in_to_account(credentials).await
                            };
                            match result {
                                Ok(issued) => {
                                    let switched = !upgrade;
                                    session.adopt(issued);
                                    password.set(String::new());
                                    signing_in.set(false);
                                    app_state.show_toast(
                                        if switched { "Signed in" } else { "Account created" },
                                        StatusColor::Success,
                                    );
                                    // A different account has a different
                                    // library, and this process is holding the
                                    // previous one in memory.
                                    if switched {
                                        reload_client();
                                    }
                                }
                                Err(server_error) => error.set(readable(&server_error.to_string())),
                            }
                            busy.set(false);
                        });
                    },
                    if upgrade { "Create account" } else { "Sign in" }
                }
                if is_guest {
                    Button {
                        style: ButtonStyle::Neutral,
                        expand: true,
                        onclick: move |_| {
                            signing_in.toggle();
                            error.set(String::new());
                        },
                        if signing_in() {
                            "Back to creating an account"
                        } else {
                            "I already have an account"
                        }
                    }
                    if signing_in() {
                        p { class: "account-warning",
                            "Signing in to another account leaves this guest library behind. It is not merged."
                        }
                    }
                }
            } else {
                Button {
                    style: ButtonStyle::Neutral,
                    expand: true,
                    onclick: move |_| {
                        spawn(async move {
                            // Best effort: a server that cannot be reached
                            // still has to leave this device signed out, so
                            // the local token is dropped either way.
                            let _ = sign_out_of_account().await;
                            session.forget();
                            reload_client();
                        });
                    },
                    "Sign out"
                }
            }
        }
    }
}

/// The server section of the settings page.
#[component]
pub fn BackendSettings() -> Element {
    let url = use_signal(config::backend_url);
    let mut error = use_signal(String::new);
    let mut saved = use_signal(|| false);

    rsx! {
        Card {
            h2 { "Server" }
            Field {
                label: "Server address".to_string(),
                value: url,
                r#type: "url".to_string(),
                oninput: move |_| {
                    error.set(String::new());
                    saved.set(false);
                },
            }
            if !error().is_empty() {
                p { class: "account-error", "{error}" }
            }
            if saved() {
                p { class: "account-warning",
                    "Saved. Restarting to connect to it - on a packaged build, close and reopen the app."
                }
            }
            Button {
                style: ButtonStyle::Neutral,
                expand: true,
                onclick: move |_| {
                    match config::set_backend_url(&url()) {
                        Some(_) => {
                            saved.set(true);
                            // The signed-in account belongs to the *old*
                            // server, so its token means nothing to the new
                            // one. Dropping it lets the next launch mint a
                            // guest there instead of failing to authenticate.
                            config::clear_session_token();
                            reload_client();
                        }
                        None => error.set(
                            "Enter a full address, including http:// or https://.".into(),
                        ),
                    }
                },
                "Save and reconnect"
            }
        }
    }
}
