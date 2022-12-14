use crate::error::Error;
use actix_web::{cookie::Cookie, dev::ServiceRequest};
use async_trait::async_trait;
use infrastructure::store::{
    models::user_session::UserSession,
    repository::{role::Role, session::Session},
};

#[async_trait]
pub(crate) trait AuthGuardContract {
    async fn get_valid_session(&self, session_id: &str, csrf: &str) -> Result<UserSession, Error>;
    fn get_csrf_header<'a>(&self, reg: &'a ServiceRequest) -> Result<&'a str, Error>;
    fn get_session_cookie(&self, reg: &ServiceRequest) -> Result<Cookie, Error>;
    fn check_valid_role(&self, role: &Role) -> bool;
}

#[async_trait]
pub(crate) trait RepositoryContract {
    async fn get_valid_user_session(&self, id: &str, csrf: &str) -> Result<UserSession, Error>;
    async fn refresh_session(&self, id: &str, csrf: &str) -> Result<Session, Error>;
}

#[async_trait]
pub(crate) trait CacheContract {
    async fn get_session_by_id(&self, id: &str) -> Result<UserSession, Error>;
    async fn cache_session(&self, csrf: &str, session: &UserSession) -> Result<(), Error>;
    async fn refresh_session(&self, session_id: &str) -> Result<(), Error>;
}
