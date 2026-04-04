use askama::Template;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar};

use crate::state::AppState;

const COOKIE_NAME: &str = "perspt_session";

/// Login page template
#[derive(askama::Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    error: Option<String>,
}

/// Show the login form
pub async fn login_page() -> impl IntoResponse {
    let tmpl = LoginTemplate { error: None };
    Html(tmpl.render().unwrap_or_default())
}

/// Handle login form submission
pub async fn login_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::Form(form): axum::Form<LoginForm>,
) -> impl IntoResponse {
    let Some(ref expected) = state.password else {
        // No password configured — shouldn't reach here, but allow anyway
        return (jar, Redirect::to("/")).into_response();
    };

    if form.password != *expected {
        let tmpl = LoginTemplate {
            error: Some("Invalid password".to_string()),
        };
        return (
            StatusCode::UNAUTHORIZED,
            Html(tmpl.render().unwrap_or_default()),
        )
            .into_response();
    }

    // Generate random session token
    let token: String = {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..32)
            .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
            .collect()
    };

    // Store the token in app state
    *state.session_token.lock().await = Some(token.clone());

    let cookie = Cookie::build((COOKIE_NAME, token))
        .path("/")
        .http_only(true)
        .same_site(axum_extra::extract::cookie::SameSite::Lax)
        .secure(!state.is_localhost);

    (jar.add(cookie), Redirect::to("/")).into_response()
}

#[derive(serde::Deserialize)]
pub struct LoginForm {
    password: String,
}

/// Auth middleware — checks cookie against stored session token.
/// If no password is configured, all requests pass through.
pub async fn auth_middleware(
    State(state): State<AppState>,
    jar: CookieJar,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    // No password = open access
    if state.password.is_none() {
        return next.run(request).await;
    }

    // Check for valid session cookie
    if let Some(cookie) = jar.get(COOKIE_NAME) {
        let stored = state.session_token.lock().await;
        if let Some(ref token) = *stored {
            if cookie.value() == token {
                return next.run(request).await;
            }
        }
    }

    Redirect::to("/login").into_response()
}
