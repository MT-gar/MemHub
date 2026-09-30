//! Request guard for browser mode (`memhub serve`).
//!
//! The HTTP API can read and change everything in the vault, so a web page the user happens
//! to visit must not be able to drive it:
//!
//! * **DNS rebinding** – when bound to a loopback address the `Host` header must name a
//!   loopback host (`localhost`, `127.0.0.1`, `[::1]`).
//! * **Cross-site requests** – a request carrying an `Origin` header must come from an
//!   allowed origin (this server itself, or one added with `--allow-origin`). `Origin: null`
//!   is refused.
//! * **Preflight bypass** – `POST /api/*` must be `application/json`, which browsers will not
//!   send cross-origin without a CORS preflight (and we never answer preflights).
//! * **Token** – whenever the server is reachable from other machines (non-loopback bind) or
//!   `--token` is given, every request except `/api/health` needs the token: as
//!   `Authorization: Bearer`, `X-MemHub-Token`, or the `memhub_token` cookie. Opening
//!   `/?token=…` once stores the cookie (`HttpOnly; SameSite=Strict`) and redirects.

use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

pub const COOKIE: &str = "memhub_token";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny(StatusCode, &'static str),
    /// Valid `?token=` on a page request: set the cookie and redirect to `location`.
    Bootstrap { token: String, location: String },
}

#[derive(Debug, Clone)]
pub struct Guard {
    token: Option<String>,
    check_host: bool,
    allowed_origins: Vec<String>,
}

pub fn is_loopback_host(host: &str) -> bool {
    matches!(host.trim_matches(|c| c == '[' || c == ']').to_ascii_lowercase().as_str(), "127.0.0.1" | "localhost" | "::1")
}

impl Guard {
    /// `bind_host`/`port` are the listen address; `extra_origins` come from `--allow-origin`;
    /// `token` from `--token` / `MEMHUB_TOKEN`. A token is generated for non-loopback binds.
    pub fn new(bind_host: &str, port: u16, extra_origins: &[String], token: Option<String>) -> Guard {
        let loopback = is_loopback_host(bind_host);
        let token = token.filter(|t| !t.is_empty()).or_else(|| if loopback { None } else { Some(random_token()) });
        let mut allowed_origins: Vec<String> = Vec::new();
        if loopback {
            for h in ["localhost", "127.0.0.1", "[::1]"] {
                allowed_origins.push(format!("http://{h}:{port}"));
            }
        }
        allowed_origins.extend(extra_origins.iter().map(|o| o.trim().trim_end_matches('/').to_ascii_lowercase()).filter(|o| !o.is_empty()));
        Guard { token, check_host: loopback, allowed_origins }
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub fn check(&self, method: &Method, path: &str, query: Option<&str>, headers: &HeaderMap) -> Decision {
        let host_hdr = headers.get(header::HOST).and_then(|v| v.to_str().ok()).unwrap_or("");

        if self.check_host && !is_loopback_host(host_name(host_hdr)) {
            return Decision::Deny(StatusCode::FORBIDDEN, "host not allowed");
        }

        if let Some(origin) = headers.get(header::ORIGIN) {
            let origin = origin.to_str().unwrap_or("").trim().trim_end_matches('/').to_ascii_lowercase();
            let same_origin = !host_hdr.is_empty() && (origin == format!("http://{}", host_hdr.to_ascii_lowercase()) || origin == format!("https://{}", host_hdr.to_ascii_lowercase()));
            // In loopback mode "same origin" is only trusted if the Host check above passed;
            // in token mode the token is what protects us, so same-origin is fine as well.
            if !(self.allowed_origins.contains(&origin) || same_origin && (self.token.is_some() || self.check_host)) {
                return Decision::Deny(StatusCode::FORBIDDEN, "origin not allowed");
            }
        }

        let is_api = path.starts_with("/api/");
        if is_api && method == Method::POST {
            let ct = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_ascii_lowercase();
            if !ct.starts_with("application/json") {
                return Decision::Deny(StatusCode::UNSUPPORTED_MEDIA_TYPE, "content-type must be application/json");
            }
        }

        let Some(expected) = &self.token else { return Decision::Allow };
        if path == "/api/health" {
            return Decision::Allow;
        }
        if presented_tokens(headers).any(|t| ct_eq(&t, expected)) {
            return Decision::Allow;
        }
        if !is_api && (method == Method::GET || method == Method::HEAD) {
            if let Some(q) = query_param(query, "token") {
                if ct_eq(&q, expected) {
                    return Decision::Bootstrap { token: expected.clone(), location: path.to_string() };
                }
            }
        }
        Decision::Deny(StatusCode::UNAUTHORIZED, "token required: open the URL printed by `memhub serve` (it contains ?token=…)")
    }
}

/// Axum middleware: apply [`Guard::check`] to every request.
pub async fn enforce(State(g): State<Arc<Guard>>, req: Request, next: Next) -> Response {
    let d = g.check(req.method(), req.uri().path(), req.uri().query(), req.headers());
    match d {
        Decision::Allow => next.run(req).await,
        Decision::Deny(code, msg) => (code, msg).into_response(),
        Decision::Bootstrap { token, location } => {
            let mut res = Response::builder().status(StatusCode::SEE_OTHER).header(header::LOCATION, location).body(axum::body::Body::empty()).unwrap();
            if let Ok(v) = HeaderValue::from_str(&format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict")) {
                res.headers_mut().insert(header::SET_COOKIE, v);
            }
            res
        }
    }
}

fn host_name(host_header: &str) -> &str {
    let h = host_header.trim();
    if let Some(rest) = h.strip_prefix('[') {
        // [::1]:7337
        return rest.split(']').next().unwrap_or(rest);
    }
    h.rsplit_once(':').map(|(n, p)| if p.chars().all(|c| c.is_ascii_digit()) { n } else { h }).unwrap_or(h)
}

fn presented_tokens(headers: &HeaderMap) -> impl Iterator<Item = String> + '_ {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")))
        .map(|s| s.trim().to_string());
    let custom = headers.get("x-memhub-token").and_then(|v| v.to_str().ok()).map(|s| s.trim().to_string());
    let cookie = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()).and_then(|c| {
        c.split(';').filter_map(|kv| kv.trim().split_once('=')).find(|(k, _)| *k == COOKIE).map(|(_, v)| v.to_string())
    });
    [bearer, custom, cookie].into_iter().flatten()
}

fn query_param(query: Option<&str>, key: &str) -> Option<String> {
    query?.split('&').filter_map(|kv| kv.split_once('=')).find(|(k, _)| *k == key).map(|(_, v)| v.to_string())
}

/// Constant-time equality (tokens are short; avoids early-exit timing differences).
fn ct_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut diff = (a.len() ^ b.len()) as u8;
    for i in 0..a.len().max(b.len()) {
        diff |= a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0);
    }
    diff == 0
}

/// 192 bits from the OS RNG, hex-encoded.
pub fn random_token() -> String {
    let mut buf = [0u8; 24];
    getrandom::fill(&mut buf).expect("OS random number generator unavailable");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hdrs(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(header::HeaderName::from_bytes(k.as_bytes()).unwrap(), HeaderValue::from_str(v).unwrap());
        }
        h
    }
    const JSON: (&str, &str) = ("content-type", "application/json");

    fn loopback() -> Guard {
        Guard::new("127.0.0.1", 7337, &[], None)
    }

    #[test]
    fn loopback_allows_same_origin_ui_and_plain_requests() {
        let g = loopback();
        assert!(g.token().is_none());
        for host in ["127.0.0.1:7337", "localhost:7337", "[::1]:7337", "LOCALHOST:7337"] {
            assert_eq!(g.check(&Method::GET, "/", None, &hdrs(&[("host", host)])), Decision::Allow, "{host}");
        }
        // UI fetch: same-origin POST with Origin + JSON
        let h = hdrs(&[("host", "localhost:7337"), ("origin", "http://localhost:7337"), JSON]);
        assert_eq!(g.check(&Method::POST, "/api/get_overview", None, &h), Decision::Allow);
        // curl / CLI clients send no Origin
        let h = hdrs(&[("host", "127.0.0.1:7337"), JSON]);
        assert_eq!(g.check(&Method::POST, "/api/list", None, &h), Decision::Allow);
    }

    #[test]
    fn dns_rebinding_is_refused() {
        let g = loopback();
        let h = hdrs(&[("host", "evil.example.com:7337"), ("origin", "http://evil.example.com:7337"), JSON]);
        assert_eq!(g.check(&Method::POST, "/api/save_config", None, &h), Decision::Deny(StatusCode::FORBIDDEN, "host not allowed"));
        let h = hdrs(&[("host", "localhost.evil.com"), JSON]);
        assert!(matches!(g.check(&Method::GET, "/", None, &h), Decision::Deny(StatusCode::FORBIDDEN, _)));
        // no Host header at all (HTTP/1.0 style) is not loopback either
        assert!(matches!(g.check(&Method::GET, "/", None, &HeaderMap::new()), Decision::Deny(StatusCode::FORBIDDEN, _)));
    }

    #[test]
    fn cross_site_requests_are_refused() {
        let g = loopback();
        for origin in ["https://evil.example", "null", "http://localhost:9999", "http://127.0.0.1"] {
            let h = hdrs(&[("host", "127.0.0.1:7337"), ("origin", origin), JSON]);
            assert_eq!(g.check(&Method::POST, "/api/save_config", None, &h), Decision::Deny(StatusCode::FORBIDDEN, "origin not allowed"), "{origin}");
        }
    }

    #[test]
    fn simple_form_posts_need_json_content_type() {
        let g = loopback();
        for ct in ["text/plain", "application/x-www-form-urlencoded", "multipart/form-data; boundary=x"] {
            let h = hdrs(&[("host", "localhost:7337"), ("content-type", ct)]);
            assert!(matches!(g.check(&Method::POST, "/api/save_config", None, &h), Decision::Deny(StatusCode::UNSUPPORTED_MEDIA_TYPE, _)), "{ct}");
        }
        let h = hdrs(&[("host", "localhost:7337")]);
        assert!(matches!(g.check(&Method::POST, "/api/save_config", None, &h), Decision::Deny(StatusCode::UNSUPPORTED_MEDIA_TYPE, _)));
        // charset parameter is fine
        let h = hdrs(&[("host", "localhost:7337"), ("content-type", "application/json; charset=utf-8")]);
        assert_eq!(g.check(&Method::POST, "/api/x", None, &h), Decision::Allow);
    }

    #[test]
    fn allow_origin_supports_the_vite_dev_proxy() {
        let g = Guard::new("127.0.0.1", 7337, &["http://localhost:1420/".into()], None);
        let h = hdrs(&[("host", "localhost:1420"), ("origin", "http://localhost:1420"), JSON]);
        assert_eq!(g.check(&Method::POST, "/api/get_overview", None, &h), Decision::Allow);
    }

    #[test]
    fn non_loopback_bind_generates_and_requires_a_token() {
        let g = Guard::new("0.0.0.0", 7337, &[], None);
        let t = g.token().expect("generated").to_string();
        assert_eq!(t.len(), 48);
        let base = [("host", "192.168.1.20:7337")];
        // no token -> 401, health stays open
        assert!(matches!(g.check(&Method::GET, "/", None, &hdrs(&base)), Decision::Deny(StatusCode::UNAUTHORIZED, _)));
        assert_eq!(g.check(&Method::GET, "/api/health", None, &hdrs(&base)), Decision::Allow);
        // wrong token
        let h = hdrs(&[base[0], ("authorization", "Bearer nope"), JSON]);
        assert!(matches!(g.check(&Method::POST, "/api/list", None, &h), Decision::Deny(StatusCode::UNAUTHORIZED, _)));
        // the three accepted ways
        let bearer = format!("Bearer {t}");
        let cookie = format!("a=b; {COOKIE}={t}");
        for extra in [("authorization", bearer.as_str()), ("x-memhub-token", t.as_str()), ("cookie", cookie.as_str())] {
            let h = hdrs(&[base[0], extra, JSON]);
            assert_eq!(g.check(&Method::POST, "/api/list", None, &h), Decision::Allow, "{extra:?}");
        }
        // same-origin UI requests from a LAN host are fine once the cookie is there
        let h = hdrs(&[base[0], ("origin", "http://192.168.1.20:7337"), ("cookie", cookie.as_str()), JSON]);
        assert_eq!(g.check(&Method::POST, "/api/list", None, &h), Decision::Allow);
        // a foreign origin is still refused even with a valid token
        let h = hdrs(&[base[0], ("origin", "http://evil.example"), ("cookie", cookie.as_str()), JSON]);
        assert!(matches!(g.check(&Method::POST, "/api/list", None, &h), Decision::Deny(StatusCode::FORBIDDEN, _)));
    }

    #[test]
    fn token_in_query_only_bootstraps_a_cookie_on_page_loads() {
        let g = Guard::new("0.0.0.0", 7337, &[], Some("s3cret".into()));
        let h = hdrs(&[("host", "nas.local:7337")]);
        assert_eq!(
            g.check(&Method::GET, "/", Some("token=s3cret"), &h),
            Decision::Bootstrap { token: "s3cret".into(), location: "/".into() }
        );
        // never accepted for API calls (would leak into logs / referers)
        assert!(matches!(g.check(&Method::GET, "/api/whatever", Some("token=s3cret"), &h), Decision::Deny(StatusCode::UNAUTHORIZED, _)));
        assert!(matches!(g.check(&Method::GET, "/", Some("token=wrong"), &h), Decision::Deny(StatusCode::UNAUTHORIZED, _)));
    }

    #[test]
    fn explicit_token_on_loopback_is_enforced_too() {
        let g = Guard::new("localhost", 7337, &[], Some("abc".into()));
        let h = hdrs(&[("host", "localhost:7337")]);
        assert!(matches!(g.check(&Method::GET, "/", None, &h), Decision::Deny(StatusCode::UNAUTHORIZED, _)));
        let h = hdrs(&[("host", "localhost:7337"), ("x-memhub-token", "abc")]);
        assert_eq!(g.check(&Method::GET, "/", None, &h), Decision::Allow);
    }

    #[test]
    fn helpers() {
        assert_eq!(host_name("localhost:7337"), "localhost");
        assert_eq!(host_name("[::1]:7337"), "::1");
        assert_eq!(host_name("example.com"), "example.com");
        assert!(ct_eq("abc", "abc") && !ct_eq("abc", "abd") && !ct_eq("abc", "ab") && !ct_eq("", "a"));
        assert_ne!(random_token(), random_token());
    }
}
