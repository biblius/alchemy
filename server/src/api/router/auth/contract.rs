use super::data::{
    ChangePassword, Credentials, EmailToken, ForgotPassword, ForgotPasswordVerify, Logout, Otp,
    RegistrationData, ResendRegToken, ResetPassword,
};

use crate::{error::Error, helpers::cache::CacheId};
use actix_web::HttpResponse;
use async_trait::async_trait;
use infrastructure::store::{
    models::user_session::UserSession,
    repository::{session::Session, user::User},
};
use serde::{de::DeserializeOwned, Serialize};

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(super) trait ServiceContract {
    /// Verify the user's email and password and establish a session if they don't have 2FA. If the `remember`
    /// flag is true the session established will be permanent (applies for `verify_otp` as well).
    async fn login(&self, credentails: Credentials) -> Result<HttpResponse, Error>;
    /// Verify the user's OTP and if successful establish a session.
    async fn verify_otp(&self, credentails: Otp) -> Result<HttpResponse, Error>;
    /// Start the registration process and send a registration token via email.
    async fn start_registration(&self, data: RegistrationData) -> Result<HttpResponse, Error>;
    /// Verify the registration token.
    async fn verify_registration_token(&self, data: EmailToken) -> Result<HttpResponse, Error>;
    /// Resend a registration token in case the user's initial one expired.
    async fn resend_registration_token(&self, data: ResendRegToken) -> Result<HttpResponse, Error>;
    /// Set the user's OTP secret and enable 2FA for the user. Send a QR code of the secret in the
    /// response. Requires an established session beforehand as it is not idempotent, meaning
    /// it will generate a new OTP secret every time this URL is called.
    async fn set_otp_secret(&self, user_id: &str) -> Result<HttpResponse, Error>;
    /// Change the user's password, purge all their sessions and notify by email. Sets a
    /// temporary PW token in the cache. Works only with an established session.
    async fn change_password(
        &self,
        session: UserSession,
        data: ChangePassword,
    ) -> Result<HttpResponse, Error>;
    /// Verify a token sent to a user via email when they request a forgotten password and change their
    /// password to the given one
    async fn verify_forgot_password(
        &self,
        data: ForgotPasswordVerify,
    ) -> Result<HttpResponse, Error>;
    /// Reset the user's password and send it to their email. Works only if a temporary PW
    /// token is in the cache.
    async fn reset_password(&self, data: ResetPassword) -> Result<HttpResponse, Error>;
    /// Reset the user's password
    async fn forgot_password(&self, data: ForgotPassword) -> Result<HttpResponse, Error>;
    /// Log the user out, i.e. expire their current session and purge the rest if the user
    /// selected the purge option
    async fn logout(&self, session: UserSession, data: Logout) -> Result<HttpResponse, Error>;
    /// Expire and remove from the cache all user sessions
    async fn purge_sessions<'a>(&self, user_id: &str, skip: Option<&'a str>) -> Result<(), Error>;
    /// Generate a successful authentication response and set the necessary cookies and backend session data
    async fn session_response(&self, user: User, remember: bool) -> Result<HttpResponse, Error>;
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(super) trait RepositoryContract {
    async fn create_user(&self, email: &str, username: &str, password: &str)
        -> Result<User, Error>;
    async fn get_user_by_id(&self, id: &str) -> Result<User, Error>;
    async fn get_user_by_email(&self, email: &str) -> Result<User, Error>;
    async fn freeze_user(&self, id: &str) -> Result<User, Error>;
    async fn update_user_password(&self, id: &str, hashed_pw: &str) -> Result<User, Error>;
    async fn update_email_verified_at(&self, id: &str) -> Result<User, Error>;
    async fn set_user_otp_secret(&self, id: &str, secret: &str) -> Result<User, Error>;
    async fn create_session(
        &self,
        user: &User,
        csrf_token: &str,
        permanent: bool,
    ) -> Result<Session, Error>;
    async fn expire_session(&self, session_id: &str) -> Result<Session, Error>;
    async fn purge_sessions<'a>(
        &self,
        user_id: &str,
        skip: Option<&'a str>,
    ) -> Result<Vec<Session>, Error>;
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(super) trait CacheContract {
    async fn set_session(&self, session_id: &str, session: &UserSession) -> Result<(), Error>;
    async fn set_token<T: Serialize + Sync + Send + 'static>(
        &self,
        cache_id: CacheId,
        key: &str,
        value: &T,
        ex: Option<usize>,
    ) -> Result<(), Error>;
    async fn get_token<T: DeserializeOwned + Sync + Send + 'static>(
        &self,
        cache_id: CacheId,
        key: &str,
    ) -> Result<T, Error>;
    async fn delete_token(&self, cache_id: CacheId, key: &str) -> Result<(), Error>;
    async fn cache_login_attempt(&self, user_id: &str) -> Result<u8, Error>;
    async fn delete_login_attempts(&self, user_id: &str) -> Result<(), Error>;
    async fn cache_otp_throttle(&self, user_id: &str) -> Result<i64, Error>;
    async fn delete_otp_throttle(&self, user_id: &str) -> Result<(), Error>;
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(super) trait EmailContract {
    async fn send_registration_token(
        &self,
        token: &str,
        username: &str,
        email: &str,
    ) -> Result<(), Error>;
    async fn alert_password_change(
        &self,
        username: &str,
        email: &str,
        token: &str,
    ) -> Result<(), Error>;
    async fn send_reset_password(
        &self,
        username: &str,
        email: &str,
        temp_pw: &str,
    ) -> Result<(), Error>;
    async fn send_forgot_password(
        &self,
        username: &str,
        email: &str,
        token: &str,
    ) -> Result<(), Error>;
    async fn send_freeze_account(
        &self,
        username: &str,
        email: &str,
        token: &str,
    ) -> Result<(), Error>;
}
