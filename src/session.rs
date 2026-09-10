//! Who the client is acting as, established before anything asks the server
//! for data.
//!
//! This lives above the UI rather than inside it because of ordering: the
//! library is per account now, so `get_library` answers 401 until a session
//! exists. `AppStateProvider` bootstraps the session here and holds its own
//! sync until it is [`SessionStatus::Ready`].

use dioxus::prelude::*;

use crate::{
    api::{create_guest_account, current_account},
    config,
    models::{Account, AuthSession},
};

/// Where the session is in its bootstrap.
///
/// `Failed` is a first-class state rather than a toast: a client pointed at an
/// unreachable backend cannot proceed, and the only useful next action is to
/// change the address - so the message has to reach the screen that owns that
/// input.
#[derive(Clone, Debug, PartialEq)]
pub enum SessionStatus {
    Connecting,
    Ready,
    Failed(String),
}

#[derive(Clone, Copy)]
pub struct Session {
    pub account: Signal<Option<Account>>,
    pub status: Signal<SessionStatus>,
}

impl Session {
    /// Adopt a freshly issued session: persist the token, put it on every
    /// subsequent request, and record who it belongs to.
    pub fn adopt(&mut self, issued: AuthSession) {
        config::set_session_token(&issued.token);
        config::install_session_header(&issued.token);
        self.account.set(Some(issued.account));
        self.status.set(SessionStatus::Ready);
    }

    pub fn forget(&mut self) {
        config::clear_session_token();
        config::clear_session_header();
        self.account.set(None);
    }

    pub fn is_ready(&self) -> bool {
        (self.status)() == SessionStatus::Ready
    }
}

/// Create the session context and bootstrap it.
///
/// Called once, from `AppStateProvider`, so that every consumer below it -
/// including the library sync in that same component - can rely on the account
/// existing before it asks for anything.
pub fn use_session_provider() -> Session {
    let mut session = use_context_provider(|| Session {
        account: Signal::new(None),
        status: Signal::new(SessionStatus::Connecting),
    });
    let mut started = use_signal(|| false);

    use_effect(move || {
        if started() {
            return;
        }
        started.set(true);
        spawn(async move {
            // A stored token is worth trying before minting anything: it is the
            // difference between resuming an account and silently starting a
            // second empty one beside it.
            if config::session_token().is_some() {
                match current_account().await {
                    Ok(account) => {
                        session.account.set(Some(account));
                        session.status.set(SessionStatus::Ready);
                        return;
                    }
                    // Expired, or issued by a different backend. Falling through
                    // to mint a guest is right for the first; for the second the
                    // old library simply stays on the server that holds it.
                    Err(_) => session.forget(),
                }
            }
            match create_guest_account().await {
                Ok(issued) => session.adopt(issued),
                Err(error) => session
                    .status
                    .set(SessionStatus::Failed(readable(&error.to_string()))),
            }
        });
    });

    session
}

pub fn use_session() -> Session {
    use_context::<Session>()
}

/// Server errors arrive as transport prose. This keeps the one case a viewer
/// can actually act on - not reaching the server at all - legible.
pub fn readable(error: &str) -> String {
    let lowered = error.to_lowercase();
    if lowered.contains("error sending request")
        || lowered.contains("failed to fetch")
        || lowered.contains("connect")
        || lowered.contains("network")
    {
        "Could not reach that server. Check the address and that it is running.".into()
    } else {
        error.to_string()
    }
}
