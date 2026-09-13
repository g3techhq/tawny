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
    models::Account,
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
    /// The server wrote the cookie; retain only the account display state.
    pub fn adopt(&mut self, account: Account) {
        self.account.set(Some(account));
        self.status.set(SessionStatus::Ready);
    }

    pub fn forget(&mut self) {
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
            if let Ok(account) = current_account().await {
                session.adopt(account);
                return;
            }
            match create_guest_account().await {
                Ok(account) => session.adopt(account),
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
