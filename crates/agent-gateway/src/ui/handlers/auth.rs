//! Mock auth handlers — login form, login submit, logout.

use crate::ui::session::{self, COOKIE_NAME};
use crate::ui::view::{time_greeting, LoginView};
use askama::Template;
use axum::{
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use time::Duration;
use serde::Deserialize;

pub async fn login_get(jar: CookieJar) -> Response {
    if let Some(c) = jar.get(COOKIE_NAME) {
        if session::verify(c.value()).is_some() {
            return Redirect::to("/").into_response();
        }
    }
    let view = LoginView {
        title: "Enter",
        year: chrono::Utc::now().format("%Y").to_string().parse().unwrap_or(2026),
        greeting_eyebrow: format!("{} · ConusAI workshop", time_greeting()),
        default_name: "John Smith".into(),
        error: None,
    };
    Html(view.render().unwrap_or_else(|e| format!("<pre>{e}</pre>"))).into_response()
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub name: String,
    #[serde(default = "default_plan")]
    pub plan: String,
}

fn default_plan() -> String {
    "enterprise".into()
}

pub async fn login_post(jar: CookieJar, Form(form): Form<LoginForm>) -> Response {
    let name = form.name.trim();
    if name.is_empty() || name.len() > 60 {
        let view = LoginView {
            title: "Enter",
            year: chrono::Utc::now().format("%Y").to_string().parse().unwrap_or(2026),
            greeting_eyebrow: format!("{} · ConusAI workshop", time_greeting()),
            default_name: name.into(),
            error: Some("Name must be between 1 and 60 characters.".into()),
        };
        return Html(view.render().unwrap_or_default()).into_response();
    }
    let token = session::sign(name, &form.plan);
    let mut cookie = Cookie::new(COOKIE_NAME, token);
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_path("/");
    cookie.set_max_age(Duration::hours(24));
    (jar.add(cookie), Redirect::to("/")).into_response()
}

pub async fn logout(jar: CookieJar) -> Response {
    let mut cookie = Cookie::new(COOKIE_NAME, "");
    cookie.set_path("/");
    cookie.set_max_age(Duration::ZERO);
    (jar.remove(cookie), Redirect::to("/login")).into_response()
}
