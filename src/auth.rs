//! Accounts and cookie-backed Axum sessions.
//!
//! First launch creates a guest account and logs it into an HTTP-only session
//! cookie. Registration promotes that same account; sign-in replaces the
//! session's user. No credential is kept in Tawny's JavaScript state.

mod session_store;

use std::sync::Arc;

use anyhow::{Result, anyhow};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use axum_session_auth::{AuthSession, Authentication};
use serde::Deserialize;
use surrealdb::{Surreal, engine::any::Any};
use surrealdb_types::SurrealValue;

use crate::models::{Account, CredentialProblem, validate_email, validate_password};

pub use session_store::SurrealSessionPool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    Credential(CredentialProblem),
    InvalidLogin,
    AlreadyRegistered,
}

impl AuthError {
    pub fn message(&self) -> String {
        match self {
            Self::Credential(problem) => problem.message().to_string(),
            Self::InvalidLogin => "That email and password do not match an account.".into(),
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

#[derive(Debug, Deserialize, SurrealValue)]
struct DbAccount {
    id: String,
    display_name: String,
    email: Option<String>,
    is_guest: Option<bool>,
}

impl From<DbAccount> for Account {
    fn from(row: DbAccount) -> Self {
        Self {
            id: row.id,
            display_name: row.display_name,
            is_guest: row.is_guest.unwrap_or(row.email.is_none()),
            email: row.email,
        }
    }
}

#[derive(Debug, Deserialize, SurrealValue)]
struct DbCredential {
    id: String,
    display_name: String,
    email: Option<String>,
    password_hash: Option<String>,
}

const ACCOUNT_COLUMNS: &str = "type::string(id) AS id, display_name, email, is_guest";

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

pub struct Accounts<'db> {
    db: &'db Surreal<Any>,
}

impl<'db> Accounts<'db> {
    pub fn new(db: &'db Surreal<Any>) -> Self {
        Self { db }
    }

    pub async fn create_guest(&self) -> Result<Account> {
        let created: Vec<DbAccount> = self
            .db
            .query(format!(
                r#"CREATE app_user SET
                    display_name = "Guest", email = NONE, password_hash = NONE, is_guest = true
                RETURN {ACCOUNT_COLUMNS}"#
            ))
            .await?
            .check()?
            .take(0)?;
        created
            .into_iter()
            .next()
            .map(Into::into)
            .ok_or_else(|| anyhow!("guest account was not created"))
    }

    pub async fn register(&self, account_id: &str, email: &str, password: &str) -> Result<Account> {
        let account = self.find(account_id).await?;
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
                    email = $email, password_hash = $password_hash, display_name = $email,
                    is_guest = false, updated_at = time::now()
                RETURN {ACCOUNT_COLUMNS}"#
            ))
            .bind(("id", account.id))
            .bind(("email", email))
            .bind(("password_hash", password_hash))
            .await?
            .check()
            .map_err(|_| AuthError::Credential(CredentialProblem::EmailTaken))?
            .take(0)?;
        updated
            .into_iter()
            .next()
            .map(Into::into)
            .ok_or_else(|| anyhow!("account row vanished during sign-up"))
    }

    pub async fn sign_in(&self, email: &str, password: &str) -> Result<Account> {
        let candidates: Vec<DbCredential> = self
            .db
            .query(format!(
                "SELECT {ACCOUNT_COLUMNS}, password_hash FROM app_user WHERE email = $email LIMIT 1"
            ))
            .bind(("email", email.trim().to_lowercase()))
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
        Ok(Account {
            id: candidate.id,
            display_name: candidate.display_name,
            email: candidate.email,
            is_guest: false,
        })
    }

    pub async fn find(&self, account_id: &str) -> Result<Account> {
        let accounts: Vec<DbAccount> = self
            .db
            .query(format!(
                "SELECT {ACCOUNT_COLUMNS} FROM app_user WHERE id = type::record($id) LIMIT 1"
            ))
            .bind(("id", account_id.to_string()))
            .await?
            .check()?
            .take(0)?;
        accounts
            .into_iter()
            .next()
            .map(Into::into)
            .ok_or_else(|| anyhow!("account not found"))
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
}

#[derive(Clone, Debug)]
pub struct SessionUser {
    id: String,
    anonymous: bool,
}

impl Default for SessionUser {
    fn default() -> Self {
        Self {
            id: String::new(),
            anonymous: true,
        }
    }
}

impl SessionUser {
    fn from_account(account: &Account) -> Self {
        Self {
            id: account.id.clone(),
            anonymous: false,
        }
    }
}

#[async_trait]
impl Authentication<SessionUser, String, Arc<Surreal<Any>>> for SessionUser {
    async fn load_user(userid: String, db: Option<&Arc<Surreal<Any>>>) -> Result<Self> {
        let db = db.ok_or_else(|| anyhow!("database connection not provided"))?;
        let account = Accounts::new(db.as_ref()).find(&userid).await?;
        Ok(Self::from_account(&account))
    }

    fn is_authenticated(&self) -> bool {
        !self.anonymous
    }
    fn is_active(&self) -> bool {
        !self.anonymous
    }
    fn is_anonymous(&self) -> bool {
        self.anonymous
    }
}

pub type TawnyAuthSession =
    AuthSession<SessionUser, String, SurrealSessionPool<Any>, Arc<Surreal<Any>>>;

pub struct Owner(pub String);

impl<S> FromRequestParts<S> for Owner
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        let auth_session = parts.extensions.get::<TawnyAuthSession>().ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "auth session middleware is missing",
        ))?;
        auth_session
            .current_user
            .as_ref()
            .filter(|user| user.is_authenticated())
            .map(|user| Self(user.id.clone()))
            .ok_or((
                StatusCode::UNAUTHORIZED,
                "your session has expired; please sign in again",
            ))
    }
}

pub struct SessionAuth(pub TawnyAuthSession);

impl<S> FromRequestParts<S> for SessionAuth
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<TawnyAuthSession>()
            .cloned()
            .map(Self)
            .ok_or((
                StatusCode::INTERNAL_SERVER_ERROR,
                "auth session middleware is missing",
            ))
    }
}

pub fn sign_in_session(session: &TawnyAuthSession, account: &Account) {
    session.login_user(account.id.clone());
    session.remember_user(true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_verifies_only_against_itself() {
        let encoded = hash_password("correct horse battery").expect("hash");
        assert!(password_matches("correct horse battery", &encoded));
        assert!(!password_matches("Correct horse battery", &encoded));
    }

    #[test]
    fn the_same_password_hashes_differently_each_time() {
        assert_ne!(
            hash_password("correct horse battery").expect("hash"),
            hash_password("correct horse battery").expect("hash")
        );
    }

    #[test]
    fn a_corrupt_hash_is_refused_rather_than_panicking() {
        assert!(!password_matches("anything", "not-a-phc-string"));
    }

    #[test]
    fn a_sign_in_failure_does_not_reveal_whether_the_account_exists() {
        let message = AuthError::InvalidLogin.message().to_lowercase();
        assert!(!message.contains("not found"));
    }
}
