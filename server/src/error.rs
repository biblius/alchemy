use crate::helpers::cache::CacheId;
use actix_web::{body::BoxBody, HttpResponse, HttpResponseBuilder as Response, ResponseError};
use infrastructure::clients::store::redis;
use reqwest::StatusCode;
use serde::Serialize;
use std::fmt::Display;
use thiserror::{self, Error};
use validator::{ValidationError, ValidationErrors};

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("Authentication Error: {0}")]
    Authentication(#[from] AuthenticationError),
    #[error("Client Error: {0}")]
    ClientError(#[from] infrastructure::clients::ClientError),
    #[error("Cache Error: {0}")]
    Cache(#[from] crate::helpers::cache::CacheError),
    #[error("Adapter Error: {0}")]
    Adapter(#[from] infrastructure::store::adapters::AdapterError),
    #[error("Redis Error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("Crypto Error: {0}")]
    Crypto(#[from] infrastructure::crypto::CryptoError),
    #[error("Serde Error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Reqwest Header Error: {0}")]
    HeaderValue(#[from] reqwest::header::InvalidHeaderValue),
    #[error("Reqwest Header Error: {0}")]
    ToStr(#[from] reqwest::header::ToStrError),
    #[error("Validation Error")]
    Validation(Vec<ValidationError>),
    #[error("Http Error")]
    Http(#[from] infrastructure::web::http::HttpError),
    /// Useful for testing when you need an error response
    #[error("None")]
    #[allow(dead_code)]
    None,
}

impl From<ValidationErrors> for Error {
    fn from(e: ValidationErrors) -> Self {
        let mut errors = vec![];
        Self::nest_validation_errors(e, &mut errors);
        Self::Validation(errors)
    }
}

impl Error {
    pub fn new<E: Into<Self>>(e: E) -> Self {
        e.into()
    }

    /// Returns error message and description
    pub fn message_and_description(&self) -> (&'static str, String) {
        match self {
            /*
             * Authentication
             */
            Self::Authentication(e) => match e {
                AuthenticationError::Unauthenticated => ("UNAUTHORIZED", "No session".to_string()),
                AuthenticationError::InvalidCsrfHeader => {
                    ("UNAUTHORIZED", "Invalid CSRF header".to_string())
                }
                AuthenticationError::InvalidCredentials => {
                    ("UNAUTHORIZED", "Invalid credentials".to_string())
                }
                AuthenticationError::InvalidOTP => {
                    ("UNAUTHORIZED", "Invalid OTP provided".to_string())
                }
                AuthenticationError::AccountFrozen => {
                    ("SUSPENDED", "Account suspended".to_string())
                }
                AuthenticationError::EmailTaken => {
                    ("EMAIL_TAKEN", "Cannot use provided email".to_string())
                }
                AuthenticationError::EmailUnverified => {
                    ("UNVERIFIED", "Email not verified".to_string())
                }
                AuthenticationError::AuthBlocked => {
                    ("BLOCKED", "Authentication currently blocked".to_string())
                }
                AuthenticationError::AlreadyVerified => {
                    ("VERIFIED", "Account already verified".to_string())
                }
                AuthenticationError::InsufficientRights => {
                    ("FORBIDDEN", "Insufficient rights".to_string())
                }
                AuthenticationError::InvalidToken(id) => match id {
                    CacheId::OTPToken => ("INVALID_TOKEN", "Invalid OTP token".to_string()),
                    CacheId::RegToken => {
                        ("INVALID_TOKEN", "Invalid registration token".to_string())
                    }
                    CacheId::PWToken => {
                        ("INVALID_TOKEN", "Invalid password change token".to_string())
                    }
                    _ => ("INVALID_TOKEN", "Token not found".to_string()),
                },
            },
            /*
             * Adapter
             */
            Self::Adapter(infrastructure::store::adapters::AdapterError::DoesNotExist(r)) => {
                ("NOT_FOUND", format!("Resource does not exist: {}", r))
            }

            /*
             * Validation
             */
            Self::Validation(_) => ("VALIDATION", "Invalid input".to_string()),
            _ => ("INTERNAL_SERVER_ERROR", "Internal server error".to_string()),
        }
    }

    /// Check whether the error is a validation error
    fn check_validation_errors(&self) -> Option<Vec<ValidationError>> {
        match self {
            Error::Validation(errors) => Some(errors.clone()),
            _ => None,
        }
    }

    /// Nests validation errors to one vec
    fn nest_validation_errors(errs: ValidationErrors, buff: &mut Vec<ValidationError>) {
        for err in errs.errors().values() {
            match err {
                validator::ValidationErrorsKind::Struct(box_error) => {
                    Self::nest_validation_errors(*box_error.clone(), buff);
                }
                validator::ValidationErrorsKind::List(e) => {
                    for er in e.clone().into_values() {
                        Self::nest_validation_errors(*er.clone(), buff);
                    }
                }
                validator::ValidationErrorsKind::Field(e) => {
                    for er in e {
                        buff.push(er.clone());
                    }
                }
            }
        }
    }
}

impl ResponseError for Error {
    fn status_code(&self) -> reqwest::StatusCode {
        match self {
            Error::Authentication(e) => e.status_code(),
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Transform the error to an `ErrorResponse` struct that implements actix's `ErrorResponse` trait.
    /// Flattens all validation errors to a vec, if any
    fn error_response(&self) -> HttpResponse<BoxBody> {
        let status = self.status_code();
        let (message, description) = self.message_and_description();
        let validation_errors = self.check_validation_errors();
        let error_response =
            ErrorResponse::new(status.as_u16(), message, &description, validation_errors);
        Response::new(status).json(error_response)
    }
}

#[derive(Serialize, Debug)]
pub struct ErrorResponse<'a> {
    code: u16,
    message: &'a str,
    description: &'a str,
    validation_errors: Option<Vec<ValidationError>>,
}

impl<'a> ErrorResponse<'a> {
    pub fn new(
        code: u16,
        message: &'a str,
        description: &'a str,
        validation_errors: Option<Vec<ValidationError>>,
    ) -> Self {
        Self {
            code,
            message,
            description,
            validation_errors,
        }
    }
}

impl Display for ErrorResponse<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Message: {}, Description: {}",
            self.message, self.description
        )
    }
}

#[derive(Debug, Error)]
pub(crate) enum AuthenticationError {
    #[error("Session not found")]
    Unauthenticated,
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("Invalid token")]
    InvalidToken(CacheId),
    #[error("Invalid OTP")]
    InvalidOTP,
    #[error("Invalid CSRF header")]
    InvalidCsrfHeader,
    #[error("Insufficient rights")]
    InsufficientRights,
    #[error("Account frozen")]
    AccountFrozen,
    #[error("Email taken")]
    EmailTaken,
    #[error("Already verified")]
    AlreadyVerified,
    #[error("Unverified email")]
    EmailUnverified,
    #[error("Authentication blocked")]
    AuthBlocked,
}

impl AuthenticationError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthenticated => StatusCode::UNAUTHORIZED,
            Self::InvalidCredentials => StatusCode::UNAUTHORIZED,
            Self::InvalidToken(_) => StatusCode::UNAUTHORIZED,
            Self::InsufficientRights => StatusCode::FORBIDDEN,
            Self::InvalidOTP => StatusCode::UNAUTHORIZED,
            Self::InvalidCsrfHeader => StatusCode::UNAUTHORIZED,
            Self::AccountFrozen => StatusCode::UNAUTHORIZED,
            Self::EmailTaken => StatusCode::CONFLICT,
            Self::EmailUnverified => StatusCode::UNAUTHORIZED,
            Self::AlreadyVerified => StatusCode::CONFLICT,
            Self::AuthBlocked => StatusCode::UNAUTHORIZED,
        }
    }
}
