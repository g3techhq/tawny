//! Accounts, passwords, and bearer sessions.
//!
//! Tawny is self-hosted, so an instance is whoever runs it plus whoever they
//! hand the URL to. That shapes two decisions here.
//!
//! **Every viewer is signed in as someone from the first frame.** A first
//! launch mints a guest rather than showing a sign-in wall: the app is usable
//! immediately, and nothing downstream has to handle a library with no owner.
//! Signing up later promotes that same `app_user` row in place - the library is
//! already hanging off it, so there is nothing to migrate and nothing to merge.
//!
//! **Sessions are bearer tokens, not cookies.** A packaged desktop or mobile
//! client points at an arbitrary origin, frequently a plain-http one on a LAN.
//! A cookie would need `SameSite=None; Secure` and working CORS credentials to
//! survive that, and would simply not arrive. A token in an `Authorization`
//! header behaves the same everywhere, which is what
//! `dioxus::fullstack::set_request_headers` gives the client.

use anyhow::{Result, anyhow};
use argon2::{
    Argon2,
    password_hash::{
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
        rand_core::{OsRng, RngCore},
    },
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use surrealdb::{Surreal, engine::any::Any};
use surrealdb_types::SurrealValue;

use crate::models::{Account, AuthSession, CredentialProblem, validate_email, validate_password};

/// How long a session stays valid without being seen, as a SurrealQL duration.
///
/// Long, deliberately. This is a media client people leave installed, and the
/// alternative to a long session is a guest being silently signed out of a
/// library they have no password to recover.
const SESSION_LIFETIME: &str = "400d";

/// What went wrong, in terms the client can act on.
///
/// `Credential` carries the field-level problem so a form can mark the right
/// input; the rest are whole-request outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    Credential(CredentialProblem),
    /// Deliberately does not say which half was wrong. Telling a caller that an
    /// address exists is how a sign-in form becomes an account-enumeration
    /// oracle.
    InvalidLogin,
    /// The bearer token is absent, unknown, or expired.
    NotAuthenticated,
    /// Sign-up was attempted by an account that already has an email.
    AlreadyRegistered,
}

impl AuthError {
    pub fn message(&self) -> String {
        match self {
            Self::Credential(problem) => problem.message().to_string(),
            Self::InvalidLogin => "That email and password do not match an account.".into(),
            Self::NotAuthenticated => "This session has expired. Sign in again.".into(),
            Self::AlreadyRegistered => "This account already has an email address.".into(),
        }
    }
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message())
    }
}

impl std::error::Error for AuthError {}

impl From<CredentialProblem> for AuthError {
    fn from(problem: CredentialProblem) -> Self {
        Self::Credential(problem)
    }
}

/// Record ids are read back through `type::string(id)` rather than a record
/// type, so every row here is plain data and the rest of the module never has
/// to care what a `RecordId` is.
#[derive(Debug, Deserialize, SurrealValue)]
struct DbAccount {
    id: String,
    display_name: String,
    email: Option<String>,
    /// Optional for the same reason as `DbChannel::subscription_content`: rows
    /// written before accounts existed read back as NONE, and `SurrealValue`
    /// does not honour `#[serde(default)]`. The backfill in schema.surql fills
    /// them in, but a row can still be read during the startup that adds it.
    is_guest: Option<bool>,
}

impl From<DbAccount> for Account {
    fn from(row: DbAccount) -> Self {
        // A row with no flag predates accounts, and predating accounts means it
        // was never registered - so the address is what settles it.
        let is_guest = row.is_guest.unwrap_or(row.email.is_none());
        Self {
            id: row.id,
            display_name: row.display_name,
            email: row.email,
            is_guest,
        }
    }
}

#[derive(Debug, Deserialize, SurrealValue)]
struct DbCredential {
    id: String,
    display_name: String,
    email: Option<String>,
    is_guest: Option<bool>,
    password_hash: Option<String>,
}

const ACCOUNT_COLUMNS: &str = "type::string(id) AS id, display_name, email, is_guest";

/// 256 bits of OS randomness, hex encoded.
///
/// Long enough that the digest below never has to withstand a brute force -
/// which is why a fast hash is right for tokens and wrong for the passwords
/// beside them.
fn mint_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// What actually goes in the database. A dumped `user_session` table yields
/// nothing anyone can present as a credential.
fn digest_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| anyhow!("could not hash password: {error}"))
}

fn password_matches(password: &str, encoded: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(encoded) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Everything account-shaped, kept away from the catalog code next door.
pub struct Accounts<'db> {
    db: &'db Surreal<Any>,
}

impl<'db> Accounts<'db> {
    pub fn new(db: &'db Surreal<Any>) -> Self {
        Self { db }
    }

    /// Mint an anonymous account and a session for it.
    ///
    /// This is what a first launch calls. The display name is not asked for and
    /// carries no meaning - it exists so the column is never empty, and signing
    /// up replaces it with the email.
    pub async fn create_guest(&self) -> Result<AuthSession> {
        let created: Vec<DbAccount> = self
            .db
            .query(format!(
                r#"CREATE app_user SET
                    display_name = "Guest",
                    email = NONE,
                    password_hash = NONE,
                    is_guest = true
                RETURN {ACCOUNT_COLUMNS}"#
            ))
            .await?
            .check()?
            .take(0)?;
        let account: Account = created
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("guest account was not created"))?
            .into();
        self.issue_session(account).await
    }

    /// Turn the *current* account into a real one, keeping its library.
    ///
    /// This is the upgrade path, and it is why signing up takes a session: the
    /// row already owns subscriptions, playlists and watch progress, so
    /// promoting it in place is the whole point. An account that already has an
    /// email is refused rather than silently having it changed - changing an
    /// address is a different operation with different confirmation needs, and
    /// conflating the two is how one becomes an account takeover.
    pub async fn register(&self, token: &str, email: &str, password: &str) -> Result<AuthSession> {
        let account = self.authenticate(token).await?;
        if !account.is_guest || account.email.is_some() {
            return Err(AuthError::AlreadyRegistered.into());
        }
        let email = validate_email(email).map_err(AuthError::from)?;
        validate_password(password).map_err(AuthError::from)?;
        if self.email_is_taken(&email).await? {
            return Err(AuthError::Credential(CredentialProblem::EmailTaken).into());
        }

        let password_hash = hash_password(password)?;
        let updated: Vec<DbAccount> = self
            .db
            .query(format!(
                r#"UPDATE type::record($id) SET
                    email = $email,
                    password_hash = $password_hash,
                    display_name = $email,
                    is_guest = false,
                    updated_at = time::now()
                RETURN {ACCOUNT_COLUMNS}"#
            ))
            .bind(("id", account.id.clone()))
            .bind(("email", email.clone()))
            .bind(("password_hash", password_hash))
            .await?
            // The unique index is the real arbiter. `email_is_taken` above can
            // lose a race with a simultaneous sign-up, and this is where that
            // shows up.
            .check()
            .map_err(|_| AuthError::Credential(CredentialProblem::EmailTaken))?
            .take(0)?;

        let account: Account = updated
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("account row vanished during sign-up"))?
            .into();

        // Every other device holding a token for this account was holding a
        // *guest* token for it. Signing up is the moment that stops being an
        // anonymous handle, so the rest are dropped.
        self.revoke_all_for(&account.id).await?;
        self.issue_session(account).await
    }

    /// Sign in to an existing account.
    ///
    /// Any guest session the caller was holding is abandoned - its data stays
    /// on the row it belonged to and is not merged. There is no correct way to
    /// merge two libraries, and asking a viewer to resolve conflicts on their
    /// first screen is worse than the loss.
    pub async fn sign_in(&self, email: &str, password: &str) -> Result<AuthSession> {
        // Normalized, not validated: an address already in the database that
        // this build's rules would now reject must still be able to sign in.
        let email = email.trim().to_lowercase();
        let candidates: Vec<DbCredential> = self
            .db
            .query(format!(
                "SELECT {ACCOUNT_COLUMNS}, password_hash FROM app_user WHERE email = $email LIMIT 1"
            ))
            .bind(("email", email))
            .await?
            .check()?
            .take(0)?;

        let Some(candidate) = candidates.into_iter().next() else {
            return Err(AuthError::InvalidLogin.into());
        };
        let Some(encoded) = candidate.password_hash.as_deref() else {
            return Err(AuthError::InvalidLogin.into());
        };
        if !password_matches(password, encoded) {
            return Err(AuthError::InvalidLogin.into());
        }

        self.issue_session(Account {
            id: candidate.id,
            display_name: candidate.display_name,
            // It has a verified password, so whatever the column says it is not
            // a guest.
            is_guest: false,
            email: candidate.email,
        })
        .await
    }

    /// Resolve a bearer token to its account, refusing anything expired.
    ///
    /// Also pushes the expiry back out, so a client in daily use never has its
    /// session age out from under it while one that stops being used
    /// eventually does. The renewal carries the *same* expiry guard as the
    /// lookup on purpose: without it, presenting an expired token would renew
    /// the very row that was just refused, and the next attempt with that dead
    /// token would succeed.
    pub async fn authenticate(&self, token: &str) -> Result<Account> {
        if token.is_empty() {
            return Err(AuthError::NotAuthenticated.into());
        }
        // `$session` is a protected variable name in SurrealDB and cannot be
        // bound, hence `$found`.
        let accounts: Vec<DbAccount> = self
            .db
            .query(format!(
                r#"LET $found = (SELECT user FROM user_session
                        WHERE token_hash = $token_hash AND expires_at > time::now()
                        LIMIT 1)[0];
                UPDATE user_session
                    SET last_seen_at = time::now(), expires_at = time::now() + {SESSION_LIFETIME}
                    WHERE token_hash = $token_hash AND expires_at > time::now();
                SELECT {ACCOUNT_COLUMNS} FROM app_user WHERE id = $found.user"#
            ))
            .bind(("token_hash", digest_token(token)))
            .await?
            .check()?
            .take(2)?;

        accounts
            .into_iter()
            .next()
            .map(Into::into)
            .ok_or_else(|| AuthError::NotAuthenticated.into())
    }

    /// Drop one session. Other devices keep theirs.
    pub async fn sign_out(&self, token: &str) -> Result<()> {
        self.db
            .query("DELETE user_session WHERE token_hash = $token_hash")
            .bind(("token_hash", digest_token(token)))
            .await?
            .check()?;
        Ok(())
    }

    async fn email_is_taken(&self, email: &str) -> Result<bool> {
        let existing: Vec<DbAccount> = self
            .db
            .query(format!(
                "SELECT {ACCOUNT_COLUMNS} FROM app_user WHERE email = $email LIMIT 1"
            ))
            .bind(("email", email.to_string()))
            .await?
            .check()?
            .take(0)?;
        Ok(!existing.is_empty())
    }

    async fn revoke_all_for(&self, account_id: &str) -> Result<()> {
        self.db
            .query("DELETE user_session WHERE user = type::record($id)")
            .bind(("id", account_id.to_string()))
            .await?
            .check()?;
        Ok(())
    }

    async fn issue_session(&self, account: Account) -> Result<AuthSession> {
        let token = mint_token();
        self.db
            .query(format!(
                r#"CREATE user_session SET
                    token_hash = $token_hash,
                    user = type::record($user),
                    expires_at = time::now() + {SESSION_LIFETIME}"#
            ))
            .bind(("token_hash", digest_token(&token)))
            .bind(("user", account.id.clone()))
            .await?
            .check()?;
        Ok(AuthSession { token, account })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_never_reaches_the_database_in_the_clear() {
        let token = mint_token();
        let digest = digest_token(&token);
        assert_ne!(token, digest);
        assert_eq!(digest.len(), 64, "sha256, hex encoded");
        assert_eq!(digest, digest_token(&token), "the digest is stable");
    }

    #[test]
    fn tokens_do_not_repeat() {
        let first = mint_token();
        let second = mint_token();
        assert_ne!(first, second);
        assert_eq!(first.len(), 64, "256 bits, hex encoded");
    }

    #[test]
    fn a_password_verifies_only_against_itself() {
        let encoded = hash_password("correct horse battery").expect("hash");
        assert!(password_matches("correct horse battery", &encoded));
        assert!(!password_matches("Correct horse battery", &encoded));
        assert!(!password_matches("", &encoded));
    }

    #[test]
    fn the_same_password_hashes_differently_each_time() {
        // Distinct salts. Equal hashes would mean a stolen table reveals which
        // accounts share a password.
        let first = hash_password("correct horse battery").expect("hash");
        let second = hash_password("correct horse battery").expect("hash");
        assert_ne!(first, second);
    }

    #[test]
    fn a_corrupt_hash_is_refused_rather_than_panicking() {
        assert!(!password_matches("anything", "not-a-phc-string"));
        assert!(!password_matches("anything", ""));
    }

    #[test]
    fn a_sign_in_failure_does_not_reveal_whether_the_account_exists() {
        let message = AuthError::InvalidLogin.message().to_lowercase();
        assert!(!message.contains("no account"));
        assert!(!message.contains("wrong password"));
        assert!(!message.contains("not found"));
    }

    /// The renewal in `authenticate` must be guarded the same way the lookup
    /// is. This is a string check because the guard lives in SurrealQL, and
    /// losing it is silent: authentication still fails, but the refused token
    /// is renewed and works on the next attempt.
    #[test]
    fn the_session_renewal_cannot_revive_an_expired_token() {
        let source = include_str!("auth.rs");
        let update = source
            .split("UPDATE user_session")
            .nth(1)
            .expect("the renewal statement");
        let statement = update.split(';').next().expect("statement end");
        assert!(
            statement.contains("expires_at > time::now()"),
            "the renewal dropped its expiry guard: {statement}"
        );
    }
}
