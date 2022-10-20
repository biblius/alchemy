use super::{handler, service::Authentication};
use crate::{api::middleware::auth::interceptor, models::role::Role};
use actix_web::web::{self, Data};
use infrastructure::{
    email::lettre::SmtpTransport,
    storage::{postgres::Pg, redis::Rd},
};
use std::sync::Arc;

pub(crate) fn init(
    pg: Arc<Pg>,
    rd: Arc<Rd>,
    email: Arc<SmtpTransport>,
    cfg: &mut web::ServiceConfig,
) {
    let service = Authentication::new(pg.clone(), rd.clone(), email);
    let guard = interceptor::Auth::new(pg, rd, Role::User);

    cfg.app_data(Data::new(service));

    // Credentials login
    cfg.service(web::resource("/auth/login").route(web::post().to(handler::verify_credentials)));

    // OTP login
    cfg.service(web::resource("/auth/verify-otp").route(web::post().to(handler::verify_otp)));

    // Initial registration
    cfg.service(web::resource("/auth/register").route(web::post().to(handler::start_registration)));

    // Verify registration token
    cfg.service(
        web::resource("/auth/verify-registration-token")
            .route(web::get().to(handler::verify_registration_token)),
    );

    // Set password
    cfg.service(
        web::resource("/auth/{user_id}/set-password").route(web::post().to(handler::set_password)),
    );

    // Set otp
    cfg.service(
        web::resource("/auth/set-otp")
            .route(web::get().to(handler::set_otp_secret))
            .wrap(guard),
    );
}
