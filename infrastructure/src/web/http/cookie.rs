use cookie::{time::Duration, Cookie, CookieBuilder, CookieJar, Key, SameSite, SignedJar};
use serde::Serialize;

use crate::config;

// Cookie identifiers
pub const S_ID: &str = "S_ID";

const PATH: &str = "/";
const HTTP_ONLY: bool = true;
const SECURE: bool = true;
const DOMAIN: &str = "";
const MAX_AGE: Duration = Duration::days(1);
const SAME_SITE: SameSite = SameSite::Lax;

/// Creates a session cookie in a jar that can be signed with a private key.
pub fn create_session_in_jar(session_id: &str, expire: bool, permanent: bool) -> CookieJar {
    let cookie = CookieBuilder::new(S_ID, session_id)
        .path(PATH)
        .domain(DOMAIN)
        .max_age(if expire {
            Duration::ZERO
        } else if permanent {
            Duration::MAX
        } else {
            MAX_AGE
        })
        .same_site(SAME_SITE)
        .http_only(HTTP_ONLY)
        .secure(SECURE)
        .finish()
        .into_owned();
    let mut jar = CookieJar::new();
    jar.add_original(cookie);
    jar
}

/// Creates a cookie with the given session id
pub fn create_session(session_id: &str, expire: bool, permanent: bool) -> Cookie<'_> {
    CookieBuilder::new(S_ID, session_id)
        .path(PATH)
        .domain(DOMAIN)
        .max_age(if expire {
            Duration::ZERO
        } else if permanent {
            Duration::MAX
        } else {
            MAX_AGE
        })
        .same_site(SAME_SITE)
        .http_only(HTTP_ONLY)
        .secure(SECURE)
        .finish()
}

/// Creates a cookie with the given properties.
pub fn create<'a, T: Serialize>(
    key: &'a str,
    value: &T,
    expire: bool,
) -> Result<Cookie<'a>, serde_json::Error> {
    let json = serde_json::to_string(value)?;
    Ok(CookieBuilder::new(key, json)
        .path(PATH)
        .domain(DOMAIN)
        .max_age(if expire { Duration::ZERO } else { MAX_AGE })
        .same_site(SAME_SITE)
        .http_only(HTTP_ONLY)
        .secure(SECURE)
        .finish())
}

/// Creates any kind of cookie in a jar
pub fn create_in_jar<T: Serialize>(
    key: &str,
    value: &T,
    expire: bool,
) -> Result<CookieJar, serde_json::Error> {
    let json = serde_json::to_string(value)?;
    let cookie = CookieBuilder::new(key, json)
        .path(PATH)
        .domain(DOMAIN)
        .max_age(if expire { Duration::ZERO } else { MAX_AGE })
        .same_site(SAME_SITE)
        .http_only(HTTP_ONLY)
        .secure(SECURE)
        .finish()
        .into_owned();
    let mut jar = CookieJar::new();
    jar.add_original(cookie);
    Ok(jar)
}

/// Sign a jar with the key `COOKIE_SECRET` from the env
pub fn sign_jar<J>(jar: &CookieJar) -> SignedJar<&CookieJar> {
    let secret = config::env::get("COOKIE_SECRET").expect("Cookie secret must be set");
    jar.signed(&Key::from(secret.as_bytes()))
}
