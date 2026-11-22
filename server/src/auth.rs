// src/auth.rs — cookie-session authentication for admin routes.

use crate::configuration::AdminSettings;
use crate::template::TemplateRenderer;
use actix_identity::Identity;
use actix_web::HttpMessage;
use actix_web::HttpRequest;
use actix_web::http::StatusCode;
use actix_web::{
    HttpResponse,
    web::{self, Data},
};
use serde::Serialize;

/// True when the request carries a valid admin session cookie.
pub fn is_logged_in(identity: &Option<Identity>) -> bool {
    matches!(identity, Some(id) if id.id().is_ok())
}

/// Gate browser-facing admin routes: redirects to `/login` when unauthenticated.
pub fn require_login(identity: &Option<Identity>) -> Result<(), actix_web::Error> {
    if is_logged_in(identity) {
        Ok(())
    } else {
        Err(RedirectToLogin.into())
    }
}

/// Gate API write routes: returns `401 Unauthorized` when unauthenticated.
pub fn require_api_login(identity: &Option<Identity>) -> Result<(), actix_web::Error> {
    if is_logged_in(identity) {
        Ok(())
    } else {
        Err(UnauthorizedApi.into())
    }
}

#[derive(Debug)]
struct RedirectToLogin;

impl std::fmt::Display for RedirectToLogin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "authentication required")
    }
}

impl actix_web::ResponseError for RedirectToLogin {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::SeeOther()
            .insert_header(("Location", "/login"))
            .finish()
    }
}

#[derive(Debug)]
struct UnauthorizedApi;

impl std::fmt::Display for UnauthorizedApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "authentication required")
    }
}

impl actix_web::ResponseError for UnauthorizedApi {
    fn status_code(&self) -> StatusCode {
        StatusCode::UNAUTHORIZED
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::Unauthorized().body("Unauthorized")
    }
}

fn ct_eq(a: &[u8], b: &[u8]) -> subtle::Choice {
    use subtle::ConstantTimeEq;
    if a.len() != b.len() {
        return subtle::Choice::from(0);
    }
    a.ct_eq(b)
}

/// POST /login — validate credentials in constant time, attach session identity.
pub async fn post_login(
    req: HttpRequest,
    params: web::Form<LoginForm>,
    admin: Data<AdminSettings>,
) -> HttpResponse {
    let username_ok = ct_eq(params.username.as_bytes(), admin.username.as_bytes());
    let password_ok = ct_eq(params.password.as_bytes(), admin.password.as_bytes());

    if (username_ok & password_ok).into() {
        match Identity::login(&req.extensions(), "admin".to_string()) {
            Ok(_) => HttpResponse::SeeOther()
                .insert_header(("Location", "/"))
                .finish(),
            Err(_) => HttpResponse::SeeOther()
                .insert_header(("Location", "/login?error=1"))
                .finish(),
        }
    } else {
        HttpResponse::SeeOther()
            .insert_header(("Location", "/login?error=1"))
            .finish()
    }
}

/// GET /login — render the styled login page.
pub async fn get_login_page(
    templates: Data<dyn TemplateRenderer>,
    query: web::Query<LoginQuery>,
    identity: Option<Identity>,
) -> Result<HttpResponse, actix_web::Error> {
    if is_logged_in(&identity) {
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/"))
            .finish());
    }

    #[derive(Serialize)]
    struct Context {
        title: &'static str,
        header: &'static str,
        sub_header: &'static str,
        error: bool,
        logged_in: bool,
    }

    let context = Context {
        title: "Login",
        header: "Admin Login",
        sub_header: "crusty-metallian.net",
        error: query.error.is_some(),
        logged_in: false,
    };

    let json_context = serde_json::to_value(&context)?;
    let html = templates.render("login.html", &json_context)?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

/// GET /logout — clear identity and redirect to public galleries.
pub async fn get_logout(identity: Option<Identity>) -> HttpResponse {
    if let Some(id) = identity {
        id.logout();
    }
    HttpResponse::SeeOther()
        .insert_header(("Location", "/galleries"))
        .finish()
}

#[derive(serde::Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

#[derive(serde::Deserialize)]
pub struct LoginQuery {
    pub error: Option<String>,
}
