pub(super) mod contract;
pub(super) mod data;
pub(super) mod domain;
pub(super) mod handler;
pub(super) mod infrastructure;
pub(crate) mod setup;

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        contract::{MockCacheContract, MockEmailContract, MockServiceContract, ServiceContract},
        data::{
            AuthenticationSuccessResponse, ChangePassword, Credentials, EmailToken, ForgotPassword,
            ForgotPasswordVerify, Otp, RegistrationData, ResendRegToken, ResetPassword,
        },
        domain::Authentication,
    };
    use crate::{
        api::router::auth::contract::MockRepositoryContract,
        error::{AuthenticationError, Error},
        helpers::cache::CacheId,
    };
    use actix_web::body::to_bytes;
    use chrono::NaiveDateTime;
    use data_encoding::{BASE32, BASE64URL};
    use derive_new::new;
    use infrastructure::{
        config::env,
        crypto::{
            hmac::generate_hmac,
            utility::{bcrypt_hash, uuid},
        },
        store::repository::{role::Role, session::Session, user::User},
        store::{adapters::AdapterError, models::user_session::UserSession},
        web::http::response::Response,
    };
    use lazy_static::lazy_static;
    use reqwest::StatusCode;
    use serde::{Deserialize, Serialize};

    /// Mock this one here so we don't clog the code with unnecessary implementations
    #[derive(Debug, Serialize, Deserialize, new)]
    struct TwoFactorAuthResponse {
        pub username: String,
        pub token: String,
        pub remember: bool,
    }

    lazy_static! {
        static ref CREDENTIALS: Credentials = Credentials {
            email: "test@lo.com".to_string(),
            password: "123".to_string(),
            remember: false,
        };
        static ref REGISTRATION: RegistrationData = RegistrationData {
            email: "test@lo.com".to_string(),
            password: "123".to_string(),
            username: "bibli".to_string(),
        };
        static ref USER_NO_OTP: User = User::__mock(
            uuid(),
            "bibli@khan.com",
            "bibli",
            bcrypt_hash("123").unwrap(),
            false,
            true,
            false
        );
        static ref USER_OTP: User = User::__mock(
            uuid(),
            "bibli@khan.com",
            "bibli",
            bcrypt_hash("123").unwrap(),
            true,
            true,
            false
        );
        static ref SESSION_NO_OTP: Session = Session::__mock(uuid(), &USER_NO_OTP, uuid(), false);
        static ref SESSION_OTP: Session = Session::__mock(uuid(), &USER_OTP, uuid(), true);
        static ref USER_SESSION: UserSession = UserSession {
            id: uuid(),
            csrf: uuid(),
            user_id: USER_NO_OTP.id.clone(),
            user_role: Role::User,
            user_email: USER_NO_OTP.email.clone(),
            user_name: USER_NO_OTP.username.clone(),
            user_phone: USER_NO_OTP.phone.clone(),
            frozen: false,
            google_id: None,
            github_id: None,
            expires_at: NaiveDateTime::MAX.timestamp()
        };
    }

    #[actix_web::main]
    #[test]
    async fn registration() {
        env::load_from_file("../.env").unwrap();
        /*
         * Good to go
         */
        let mut repository = MockRepositoryContract::new();
        let mut cache = MockCacheContract::new();
        let mut email = MockEmailContract::new();
        // The service will first attempt to find an existing user
        repository
            .expect_get_user_by_email()
            .return_once_st(move |_| {
                Err(Error::new(AdapterError::DoesNotExist("User".to_string())))
            });
        // Then create one
        repository
            .expect_create_user()
            .return_once(move |_, _, _| Ok(USER_NO_OTP.clone()));
        // Cache their registration token
        cache
            .expect_set_token()
            .return_once_st(move |_, _, _: &String, _| Ok(()));
        // And send it via email
        email
            .expect_send_registration_token()
            .return_once_st(move |_, _, _| Ok(()));
        let auth_service = Authentication {
            repository,
            cache,
            email,
        };
        auth_service
            .start_registration(REGISTRATION.clone())
            .await
            .unwrap();
        /*
         * Already exists
         */
        let mut repository = MockRepositoryContract::new();
        let cache = MockCacheContract::new();
        let email = MockEmailContract::new();
        repository
            .expect_get_user_by_email()
            .return_once_st(move |_| Ok(USER_NO_OTP.clone()));
        let auth_service = Authentication {
            repository,
            cache,
            email,
        };
        let res = auth_service.start_registration(REGISTRATION.clone()).await;
        match res {
            Ok(_) => panic!("Not good"),
            Err(e) => assert!(matches!(
                e,
                Error::Authentication(AuthenticationError::EmailTaken)
            )),
        }
    }

    #[actix_web::main]
    #[test]
    async fn verify_registration_token() {
        env::load_from_file("../.env").unwrap();
        /*
         * Good to go
         */
        let mut repository = MockRepositoryContract::new();
        let mut cache = MockCacheContract::new();
        let email = MockEmailContract::new();
        cache
            .expect_get_token()
            .return_once(|_, _| Ok(USER_NO_OTP.id.clone()));
        repository
            .expect_update_email_verified_at()
            .return_once(|_| Ok(USER_NO_OTP.clone()));
        cache.expect_delete_token().return_once(|_, _| Ok(()));
        let service = Authentication {
            repository,
            cache,
            email,
        };
        let data = EmailToken {
            token: generate_hmac("REG_TOKEN_SECRET", &USER_NO_OTP.id, BASE64URL).unwrap(),
        };
        service.verify_registration_token(data).await.unwrap();
        /*
         * Invalid reg token
         */
        let repository = MockRepositoryContract::new();
        let mut cache = MockCacheContract::new();
        let email = MockEmailContract::new();
        cache
            .expect_get_token::<String>()
            .return_once(|_, _| Err(Error::None));
        let service = Authentication {
            repository,
            cache,
            email,
        };
        let data = EmailToken {
            token: "12345".to_string(),
        };
        let res = service.verify_registration_token(data).await;
        match res {
            Ok(_) => panic!("Not good"),
            Err(e) => assert!(matches!(
                e,
                Error::Authentication(AuthenticationError::InvalidToken(CacheId::RegToken))
            )),
        };
    }

    #[actix_web::main]
    #[test]
    async fn resend_reg_token() {
        /*
         * Good to go
         */
        let mut repository = MockRepositoryContract::new();
        let mut cache = MockCacheContract::new();
        let mut email = MockEmailContract::new();
        let mut user = USER_NO_OTP.clone();
        user.email_verified_at = None;
        // Find the user
        repository
            .expect_get_user_by_email()
            .return_once(move |_| Ok(user));
        // See if they have an email throttle
        cache
            .expect_get_token::<i32>()
            .return_once(|_, _| Err(Error::None));
        // Set the reg token
        cache
            .expect_set_token()
            .return_once(|_, _, _: &String, _| Ok(()));
        // Send it
        email
            .expect_send_registration_token()
            .return_once(|_, _, _| Ok(()));
        // And set the email throttle
        cache
            .expect_set_token()
            .return_once(|_, _, _: &i32, _| Ok(()));
        let service = Authentication {
            repository,
            cache,
            email,
        };
        let data = ResendRegToken {
            email: USER_NO_OTP.email.clone(),
        };
        service.resend_registration_token(data).await.unwrap();
        /*
         * Already verified
         */
        let mut repository = MockRepositoryContract::new();
        let cache = MockCacheContract::new();
        let email = MockEmailContract::new();
        // Find the verified user
        repository
            .expect_get_user_by_email()
            .return_once(move |_| Ok(USER_NO_OTP.clone()));
        let service = Authentication {
            repository,
            cache,
            email,
        };
        let data = ResendRegToken {
            email: USER_NO_OTP.email.clone(),
        };
        let res = service.resend_registration_token(data).await;
        match res {
            Ok(_) => panic!("Not good"),
            Err(e) => assert!(matches!(
                e,
                Error::Authentication(AuthenticationError::AlreadyVerified)
            )),
        }
    }

    #[actix_web::main]
    #[test]
    async fn credentials_no_otp() {
        env::load_from_file("../.env").unwrap();
        let mut service = MockServiceContract::new();
        let mut repository = MockRepositoryContract::new();
        let mut cache = MockCacheContract::new();
        let email = MockEmailContract::new();
        // Find user without OTP secret
        repository
            .expect_get_user_by_email()
            .return_once(move |_| Ok(USER_NO_OTP.clone()));
        // Create session
        repository
            .expect_create_session()
            .return_once(move |_, _, _| Ok(SESSION_NO_OTP.clone()));
        // Delete login attempts
        cache.expect_delete_login_attempts().return_once(|_| Ok(()));
        // Set the session
        cache.expect_set_session().return_once(|_, _| Ok(()));
        // Respond with session
        service
            .expect_session_response()
            .return_once_st(move |_, _| {
                Ok(
                    AuthenticationSuccessResponse::new(USER_NO_OTP.clone()).to_response(
                        StatusCode::OK,
                        None,
                        None,
                    ),
                )
            });
        let auth = Authentication {
            repository,
            cache,
            email,
        };
        auth.login(CREDENTIALS.clone()).await.unwrap();
    }

    #[actix_web::main]
    #[test]
    async fn credentials_and_otp() {
        env::load_from_file("../.env").unwrap();
        let mut repository = MockRepositoryContract::new();
        let mut cache = MockCacheContract::new();
        let email = MockEmailContract::new();
        // Expect the user to exist
        repository
            .expect_get_user_by_email()
            .return_once(move |_| Ok(USER_OTP.clone()));
        // Expect to cache an OTP token
        cache
            .expect_set_token()
            .return_once(move |_, _, _: &String, _| Ok(()));
        let auth = Authentication {
            repository,
            cache,
            email,
        };
        // Verify the creds and grab the token from the response
        let res = auth.login(CREDENTIALS.clone()).await.unwrap();
        let body = to_bytes(res.into_body()).await.unwrap();
        let token =
            serde_json::from_str::<TwoFactorAuthResponse>(std::str::from_utf8(&body).unwrap())
                .unwrap()
                .token;
        let mut repository = MockRepositoryContract::new();
        let mut cache = MockCacheContract::new();
        let email = MockEmailContract::new();
        // Get the OTP token
        cache
            .expect_get_token()
            .returning(move |_, _| Ok(USER_OTP.id.clone()));
        // Try to get the OTP throttle
        cache
            .expect_get_token::<i64>()
            .return_once(move |_, _| Err(Error::None));
        // Get the user's ID stored behind the token
        repository
            .expect_get_user_by_id()
            .returning(move |_| Ok(USER_OTP.clone()));
        // Delete the OTP token
        cache.expect_delete_token().return_once(move |_, _| Ok(()));
        // Create a session
        repository
            .expect_create_session()
            .returning(move |_, _, _| Ok(SESSION_OTP.clone()));
        // Delete login attempts
        cache.expect_delete_login_attempts().return_once(|_| Ok(()));
        // Cache the session since it has the permanent flag enabled
        cache.expect_set_session().return_once(move |_, _| Ok(()));
        let auth = Authentication {
            repository,
            cache,
            email,
        };
        // Grab current time slice and calculate the OTP
        let time_step_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            / 30;
        let sec = USER_OTP.otp_secret.clone();
        let otp = thotp::otp(
            &BASE32.decode(sec.clone().unwrap().as_bytes()).unwrap(),
            time_step_now,
        )
        .unwrap();
        let data = Otp {
            password: otp,
            token,
            remember: true,
        };
        auth.verify_otp(data).await.unwrap();
    }

    #[actix_web::main]
    #[test]
    async fn invalid_credentails() {
        /*
         * Invalid email
         */
        let mut repository = MockRepositoryContract::new();
        let cache = MockCacheContract::new();
        let email = MockEmailContract::new();
        repository
            .expect_get_user_by_email()
            .return_once(move |_| Err(AuthenticationError::InvalidCredentials.into()));
        let invalid_email = Credentials {
            email: "doesnt@exist.ever".to_string(),
            password: "not good".to_string(),
            remember: false,
        };
        let service = Authentication {
            repository,
            cache,
            email,
        };
        let res = service.login(invalid_email).await;
        match res {
            Ok(_) => panic!("Not good"),
            Err(e) => assert!(matches!(
                e,
                Error::Authentication(AuthenticationError::InvalidCredentials)
            )),
        }
        /*
         * Invalid password
         */
        let mut repository = MockRepositoryContract::new();
        let mut cache = MockCacheContract::new();
        let email = MockEmailContract::new();
        // Try to find a valid user with an invalid password
        repository
            .expect_get_user_by_email()
            .return_once(move |_| Ok(USER_NO_OTP.clone()));
        cache.expect_cache_login_attempt().returning(|_| Ok(1));
        let invalid_password = Credentials {
            email: USER_NO_OTP.email.clone(),
            password: "not good".to_string(),
            remember: false,
        };
        let service = Authentication {
            repository,
            cache,
            email,
        };
        let res = service.login(invalid_password).await;
        match res {
            Ok(_) => panic!("Not good"),
            Err(e) => assert!(matches!(
                e,
                Error::Authentication(AuthenticationError::InvalidCredentials)
            )),
        }
    }

    #[actix_web::main]
    #[test]
    async fn change_password() {
        let mut service = MockServiceContract::new();
        let mut repository = MockRepositoryContract::new();
        let mut cache = MockCacheContract::new();
        let mut email = MockEmailContract::new();
        // Update pw
        repository
            .expect_update_user_password()
            .return_once(move |_, _| Ok(USER_NO_OTP.clone()));
        // Purge sessions
        service.expect_purge_sessions().return_once(|_, _| Ok(()));
        repository
            .expect_purge_sessions()
            .return_once(|_, _| Ok(vec![SESSION_NO_OTP.clone()]));
        // Delete all the cached sessions
        cache.expect_delete_token().return_once(|_, _| Ok(()));
        // Set the reset pw token
        cache
            .expect_set_token()
            .return_once(|_, _, _: &String, _| Ok(()));
        // Alert the user
        email
            .expect_alert_password_change()
            .return_once(|_, _, _| Ok(()));
        let auth = Authentication {
            repository,
            cache,
            email,
        };
        let data = ChangePassword {
            password: "12345678".to_string(),
        };
        auth.change_password(USER_SESSION.clone(), data)
            .await
            .unwrap();
    }

    #[actix_web::main]
    #[test]
    async fn reset_password() {
        /*
         * Valid token
         */
        let mut service = MockServiceContract::new();
        let mut repository = MockRepositoryContract::new();
        let mut cache = MockCacheContract::new();
        let mut email = MockEmailContract::new();
        // Expect to have a reset token
        cache
            .expect_get_token()
            .return_once(|_, _| Ok(USER_NO_OTP.id.clone()));
        // Delete the cached token
        cache.expect_delete_token().returning(|_, _| Ok(()));
        // Update the password to something random
        repository
            .expect_update_user_password()
            .return_once(|_, _| Ok(USER_NO_OTP.clone()));
        // And send it to the user
        email
            .expect_send_reset_password()
            .return_once(|_, _, _| Ok(()));
        // Purge all their sessions
        service.expect_purge_sessions().return_once(|_, _| Ok(()));
        repository
            .expect_purge_sessions()
            .return_once(|_, _| Ok(vec![SESSION_NO_OTP.clone()]));
        // Delete the cached sessions
        cache.expect_delete_token().returning(|_, _| Ok(()));
        let auth = Authentication {
            repository,
            cache,
            email,
        };
        let data = ResetPassword {
            token: "12345".to_string(),
        };
        auth.reset_password(data).await.unwrap();
        /*
         * No token
         */
        let repository = MockRepositoryContract::new();
        let mut cache = MockCacheContract::new();
        let email = MockEmailContract::new();
        cache
            .expect_get_token::<String>()
            .return_once(|_, _| Err(Error::None));
        let auth = Authentication {
            repository,
            cache,
            email,
        };
        let data = ResetPassword {
            token: "12345".to_string(),
        };
        let res = auth.reset_password(data).await;
        match res {
            Ok(_) => panic!("Not good"),
            Err(e) => assert!(matches!(
                e,
                Error::Authentication(AuthenticationError::InvalidToken(CacheId::PWToken))
            )),
        };
    }

    #[actix_web::main]
    #[test]
    async fn forgot_password() {
        let mut repository = MockRepositoryContract::new();
        let mut cache = MockCacheContract::new();
        let mut email = MockEmailContract::new();
        // Get the user
        repository
            .expect_get_user_by_email()
            .return_once(|_| Ok(USER_NO_OTP.clone()));
        // Check email throttle
        cache
            .expect_get_token::<i32>()
            .return_once(|_, _| Err(Error::None));
        // Send email
        email
            .expect_send_forgot_password()
            .return_once(|_, _, _| Ok(()));
        // Set a pw change token
        cache
            .expect_set_token()
            .returning(|_, _, _: &String, _| Ok(()));
        // Set email throttle
        cache
            .expect_set_token()
            .returning(|_, _, _: &i32, _| Ok(()));
        let service = Authentication {
            repository,
            cache,
            email,
        };
        let data = ForgotPassword {
            email: USER_NO_OTP.email.clone(),
        };
        service.forgot_password(data).await.unwrap();
        /*
         * Invalid email
         */
        let mut repository = MockRepositoryContract::new();
        let cache = MockCacheContract::new();
        let email = MockEmailContract::new();
        repository.expect_get_user_by_email().return_once(|_| {
            Err(Error::Adapter(AdapterError::DoesNotExist(
                "User".to_string(),
            )))
        });
        let service = Authentication {
            repository,
            cache,
            email,
        };
        let data = ForgotPassword {
            email: USER_NO_OTP.email.clone(),
        };
        let _msg = String::from("User");
        match service.forgot_password(data).await {
            Ok(_) => panic!("Not good"),
            Err(e) => assert!(matches!(
                e,
                Error::Adapter(AdapterError::DoesNotExist(_msg))
            )),
        };
    }

    #[actix_web::main]
    #[test]
    async fn verify_forgot_password() {
        let mut service = MockServiceContract::new();
        let mut repository = MockRepositoryContract::new();
        let mut cache = MockCacheContract::new();
        let email = MockEmailContract::new();
        // Get the user from the verify token
        cache
            .expect_get_token()
            .return_once(|_, _| Ok(USER_NO_OTP.id.clone()));
        // Delete it
        cache.expect_delete_token().return_once(|_, _| Ok(()));
        // Update the user pw
        repository
            .expect_update_user_password()
            .return_once(|_, _| Ok(USER_NO_OTP.clone()));
        // Purge all sessions
        service.expect_purge_sessions().return_once(|_, _| Ok(()));
        repository
            .expect_purge_sessions()
            .return_once(|_, _| Ok(vec![]));
        // Establish a new one
        repository
            .expect_create_session()
            .return_once(|_, _, _| Ok(SESSION_NO_OTP.clone()));
        cache.expect_delete_login_attempts().return_once(|_| Ok(()));
        cache.expect_set_session().return_once(|_, _| Ok(()));
        let auth = Authentication {
            repository,
            cache,
            email,
        };
        let data = ForgotPasswordVerify {
            password: "12345678".to_string(),
            token: "12345".to_string(),
        };
        auth.verify_forgot_password(data).await.unwrap();
        /*
         * Wrong token
         */
        let repository = MockRepositoryContract::new();
        let mut cache = MockCacheContract::new();
        let email = MockEmailContract::new();
        cache
            .expect_get_token::<String>()
            .return_once(|_, _| Err(Error::None));
        let auth = Authentication {
            repository,
            cache,
            email,
        };
        let data = ForgotPasswordVerify {
            password: "12345678".to_string(),
            token: "12345".to_string(),
        };
        let res = auth.verify_forgot_password(data).await;
        match res {
            Ok(_) => panic!("Not good"),
            Err(e) => assert!(matches!(
                e,
                Error::Authentication(AuthenticationError::InvalidToken(CacheId::PWToken))
            )),
        }
    }
}
