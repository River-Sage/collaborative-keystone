use axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, Method, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{auth::SESSION_COOKIE_NAME, error::AppError};

pub const CSRF_COOKIE_NAME: &str = "ck_csrf";
pub const CSRF_HEADER_NAME: &str = "x-csrf-token";

const PUBLIC_POST_PATHS: [&str; 5] = [
    "/auth/login",
    "/auth/register",
    "/auth/password-reset/request",
    "/auth/password-reset/confirm",
    "/bootstrap/first-moderator",
];

pub async fn validate_csrf(req: Request<Body>, next: Next) -> Response {
    if req.method() != Method::POST || PUBLIC_POST_PATHS.contains(&req.uri().path()) {
        return next.run(req).await;
    }

    let headers = req.headers();
    let Some(cookie_header) = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
    else {
        return next.run(req).await;
    };

    if extract_cookie_value(cookie_header, SESSION_COOKIE_NAME).is_none() {
        return next.run(req).await;
    }

    let csrf_cookie = extract_cookie_value(cookie_header, CSRF_COOKIE_NAME);
    let csrf_header = normalized_header_value(headers, CSRF_HEADER_NAME);

    match (csrf_cookie, csrf_header) {
        (Some(cookie), Some(header)) if constant_time_eq(cookie.as_bytes(), header.as_bytes()) => {
            next.run(req).await
        }
        _ => {
            AppError::Forbidden("Security token is invalid or missing.".to_string()).into_response()
        }
    }
}

fn normalized_header_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn extract_cookie_value<'a>(cookie_header: &'a str, cookie_name: &str) -> Option<&'a str> {
    cookie_header.split(';').find_map(|part| {
        let trimmed = part.trim();
        let (name, value) = trimmed.split_once('=')?;
        if name == cookie_name {
            Some(value)
        } else {
            None
        }
    })
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    let mut diff = 0;
    for (left_byte, right_byte) in left.iter().zip(right.iter()) {
        diff |= left_byte ^ right_byte;
    }

    diff == 0
}
