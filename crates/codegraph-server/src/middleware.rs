//! Tower middleware and HTTP plumbing: security/cache/timing headers,
//! bearer-token auth, request ids, access logs, and embedded static
//! assets with ETag handling.

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::env;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

#[allow(unused_imports)]
use crate::*;

pub(crate) async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    apply_security_headers(response.headers_mut());
    response
}

pub(crate) async fn cache_headers(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;
    apply_cache_headers_for_path(&path, response.headers_mut());
    response
}

pub(crate) async fn response_timing_header(request: Request, next: Next) -> Response {
    let started = Instant::now();
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        HeaderName::from_static(RESPONSE_TIME_HEADER),
        response_time_header_value(started.elapsed()),
    );
    response
}

/// DNS-rebinding guard, layered only when the server is bound to a loopback
/// address: a hostile site can point its own domain at 127.0.0.1 and script
/// same-origin requests against a local tool, so a loopback server must only
/// answer requests addressed to a loopback host. Requests without a Host
/// header (plain non-browser clients) are allowed — rebinding requires a
/// browser, and browsers always send Host.
pub(crate) async fn loopback_host_guard(request: Request, next: Next) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .or_else(|| request.uri().host().map(str::to_string));
    match host {
        Some(host) if !host_value_is_loopback(&host) => {
            ApiError::forbidden("this server only answers loopback hosts").into_response()
        }
        _ => next.run(request).await,
    }
}

pub(crate) fn host_value_is_loopback(value: &str) -> bool {
    let trimmed = value.trim();
    // `[::1]:port` / `[::1]`
    let host = if let Some(rest) = trimmed.strip_prefix('[') {
        match rest.split_once(']') {
            Some((inside, _)) => inside,
            None => return false,
        }
    } else {
        // `host:port` (a lone numeric port suffix) or bare `host`.
        match trimmed.rsplit_once(':') {
            Some((name, port))
                if !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()) =>
            {
                name
            }
            _ => trimmed,
        }
    };
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1"
}

pub(crate) async fn api_auth_middleware(
    State(auth): State<ApiAuth>,
    request: Request,
    next: Next,
) -> Response {
    if !request.uri().path().starts_with("/api/") || api_request_authorized(&request, &auth) {
        return next.run(request).await;
    }

    let mut response = ApiError::unauthorized("authentication required").into_response();
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        HeaderValue::from_static("Bearer realm=\"CodeGraph API\""),
    );
    response
}

#[derive(Debug, Clone)]
pub(crate) struct RequestId(pub(crate) String);

pub(crate) async fn request_id_header(mut request: Request, next: Next) -> Response {
    let request_id = incoming_request_id(&request).unwrap_or_else(next_request_id);
    request
        .extensions_mut()
        .insert(RequestId(request_id.clone()));
    let mut response = CURRENT_REQUEST_ID
        .scope(request_id.clone(), next.run(request))
        .await;
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-request-id"), value);
    }
    response
}

pub(crate) async fn access_log(request: Request, next: Next) -> Response {
    let request_id = request
        .extensions()
        .get::<RequestId>()
        .map(|request_id| request_id.0.clone())
        .unwrap_or_else(|| "-".to_string());
    let method = request.method().as_str().to_string();
    let target = request
        .uri()
        .path_and_query()
        .map(|target| target.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());
    let started = Instant::now();
    let response = next.run(request).await;
    let elapsed = started.elapsed();
    eprintln!(
        "{}",
        access_log_line(&request_id, &method, &target, response.status(), elapsed)
    );
    response
}

pub(crate) fn incoming_request_id(request: &Request) -> Option<String> {
    request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| is_valid_request_id(value))
        .map(ToOwned::to_owned)
}

pub(crate) fn configured_api_token(arg_token: Option<String>) -> Option<String> {
    arg_token
        .or_else(|| env::var("CODEGRAPH_API_TOKEN").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn api_request_authorized(request: &Request, auth: &ApiAuth) -> bool {
    let Some(expected) = auth.token.as_deref() else {
        return true;
    };
    request_api_token(request).is_some_and(|candidate| constant_time_eq(candidate, expected))
}

pub(crate) fn request_api_token(request: &Request) -> Option<&str> {
    bearer_token(request)
        .or_else(|| header_token(request, "x-codegraph-token"))
        .or_else(|| cookie_token(request))
}

pub(crate) fn bearer_token(request: &Request) -> Option<&str> {
    let value = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())?
        .trim();
    value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

pub(crate) fn header_token<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

pub(crate) fn cookie_token(request: &Request) -> Option<&str> {
    request
        .headers()
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())?
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .find_map(|(name, value)| (name.trim() == "codegraph_api_token").then_some(value.trim()))
        .filter(|token| !token.is_empty())
}

pub(crate) fn constant_time_eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.bytes()
        .zip(right.bytes())
        .fold(0_u8, |diff, (left, right)| diff | (left ^ right))
        == 0
}

pub(crate) fn next_request_id() -> String {
    format!("req-{}", NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed))
}

pub(crate) fn current_request_id() -> Option<String> {
    CURRENT_REQUEST_ID.try_with(Clone::clone).ok()
}

pub(crate) fn is_valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

pub(crate) fn access_log_line(
    request_id: &str,
    method: &str,
    target: &str,
    status: StatusCode,
    elapsed: Duration,
) -> String {
    format!(
        "{request_id} {method} {target} -> {} {}ms",
        status.as_u16(),
        elapsed.as_millis()
    )
}

pub(crate) fn response_time_header_value(elapsed: Duration) -> HeaderValue {
    HeaderValue::from_str(&elapsed.as_millis().to_string())
        .expect("elapsed milliseconds are header-safe")
}

pub(crate) fn apply_export_headers(
    headers: &mut HeaderMap,
    nodes: usize,
    edges: usize,
    bytes: usize,
) {
    headers.insert(
        HeaderName::from_static(EXPORT_NODES_HEADER),
        usize_header_value(nodes),
    );
    headers.insert(
        HeaderName::from_static(EXPORT_EDGES_HEADER),
        usize_header_value(edges),
    );
    headers.insert(
        HeaderName::from_static(EXPORT_BYTES_HEADER),
        usize_header_value(bytes),
    );
}

pub(crate) fn usize_header_value(value: usize) -> HeaderValue {
    HeaderValue::from_str(&value.to_string()).expect("usize values are header-safe")
}

pub(crate) fn apply_security_headers(headers: &mut HeaderMap) {
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(
            "default-src 'self'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:",
        ),
    );
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=()"),
    );
}

pub(crate) fn apply_cache_headers_for_path(path: &str, headers: &mut HeaderMap) {
    let value = if is_static_asset_path(path) {
        STATIC_ASSET_CACHE_CONTROL
    } else {
        DYNAMIC_CACHE_CONTROL
    };
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(value));
}

pub(crate) fn is_static_asset_path(path: &str) -> bool {
    matches!(path, "/app.js" | "/label-policy.js" | "/styles.css")
}

pub(crate) fn normalize_api_body_bytes(value: usize) -> usize {
    value.clamp(1, MAX_API_BODY_BYTES)
}

pub(crate) fn static_asset_response(
    request_headers: &HeaderMap,
    content_type: &'static str,
    body: &'static str,
) -> Response {
    let etag = static_asset_etag(body);
    let etag_header = HeaderValue::from_str(&etag).expect("static asset etags are header-safe");
    if request_headers
        .get(header::IF_NONE_MATCH)
        .is_some_and(|value| if_none_match_matches(value, &etag))
    {
        let mut response = StatusCode::NOT_MODIFIED.into_response();
        response
            .headers_mut()
            .insert(header::ETAG, etag_header.clone());
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
        return response;
    }

    let mut response = body.into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response.headers_mut().insert(header::ETAG, etag_header);
    response
}

pub(crate) fn static_asset_etag(body: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in body.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("\"codegraph-{}-{hash:016x}\"", body.len())
}

pub(crate) fn if_none_match_matches(value: &HeaderValue, etag: &str) -> bool {
    let Ok(value) = value.to_str() else {
        return false;
    };
    value.split(',').any(|candidate| {
        let candidate = candidate.trim();
        candidate == "*" || candidate == etag || candidate.strip_prefix("W/") == Some(etag)
    })
}
