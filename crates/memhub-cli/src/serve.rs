//! Browser mode: HTTP API (`POST /api/<cmd>`) + embedded web UI.

use anyhow::Result;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use crate::guard::{self, Guard};
use memhub_core::Hub;
use rust_embed::RustEmbed;
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(RustEmbed)]
#[folder = "../../ui/dist/"]
struct Assets;

pub struct Options {
    pub host: String,
    pub port: u16,
    pub watch: bool,
    /// Extra allowed `Origin`s (e.g. the Vite dev server).
    pub allow_origins: Vec<String>,
    /// Require this token (generated automatically when binding to a non-loopback address).
    pub token: Option<String>,
}

/// The router with the request guard in front of everything.
pub fn app(hub: Arc<Hub>, guard: Arc<Guard>) -> Router {
    Router::new()
        .route("/api/health", get(|| async { Json(json!({"ok": true, "version": memhub_core::VERSION})) }))
        .route("/api/{cmd}", post(rpc))
        .fallback(static_handler)
        .with_state(hub)
        .layer(axum::middleware::from_fn_with_state(guard, guard::enforce))
}

pub fn run(hub: Arc<Hub>, opts: Options) -> Result<()> {
    let Options { host, port, watch, allow_origins, token } = opts;
    let host = host.as_str();
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(async move {
        // initial sync in the background so the UI comes up immediately
        {
            let h = hub.clone();
            tokio::task::spawn_blocking(move || {
                if let Err(e) = h.sync() {
                    tracing::error!("initial sync failed: {e}");
                }
            });
        }
        let _watcher = if watch { Some(memhub_core::watch::spawn(hub.clone(), 300, None)?) } else { None };

        let guard = Arc::new(Guard::new(host, port, &allow_origins, token));
        let app = app(hub, guard.clone());
        let addr = format!("{host}:{port}");
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        let shown = if host == "0.0.0.0" || host == "::" { "localhost" } else { host };
        match guard.token() {
            Some(t) => {
                println!("MemHub UI: http://{shown}:{port}/?token={t}");
                println!("(token required: open the link above once, or send `Authorization: Bearer <token>`; keep it private)");
                if !guard::is_loopback_host(host) {
                    println!("Listening on {host}: anyone who can reach this port and has the token gets full access to your vault.");
                }
            }
            None => println!("MemHub UI: http://{shown}:{port}"),
        }
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await?;
        Ok::<(), anyhow::Error>(())
    })?;
    Ok(())
}

async fn rpc(State(hub): State<Arc<Hub>>, Path(cmd): Path<String>, body: Option<Json<Value>>) -> Response {
    let params = body.map(|b| b.0).unwrap_or(Value::Null);
    let res = tokio::task::spawn_blocking(move || memhub_core::api::dispatch(&hub, &cmd, params)).await;
    match res {
        Ok(Ok(v)) => (StatusCode::OK, Json(v)).into_response(),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match Assets::get(path).or_else(|| Assets::get("index.html")) {
        Some(file) => {
            let mime = mime_guess::from_path(if Assets::get(path).is_some() { path } else { "index.html" }).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(header::CACHE_CONTROL, if path.starts_with("assets/") { "public, max-age=31536000, immutable" } else { "no-cache" })
                .body(Body::from(file.data.into_owned()))
                .unwrap()
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{header, Method, Request};
    use tower::ServiceExt;

    struct Env {
        home: std::path::PathBuf,
        hub: Arc<Hub>,
    }
    impl Env {
        fn new() -> Env {
            // pid + counter: the clock alone is too coarse on Windows for parallel tests
            static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let home = std::env::temp_dir().join(format!("memhub-serve-test-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&home);
            let hub = Arc::new(Hub::open_at(home.clone()).unwrap());
            Env { home, hub }
        }
    }
    impl Drop for Env {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.home);
        }
    }

    fn req(method: Method, uri: &str, headers: &[(&str, &str)], body: &str) -> Request<Body> {
        let mut b = Request::builder().method(method).uri(uri);
        for (k, v) in headers {
            b = b.header(*k, *v);
        }
        b.body(Body::from(body.to_string())).unwrap()
    }

    async fn status(app: &Router, r: Request<Body>) -> StatusCode {
        app.clone().oneshot(r).await.unwrap().status()
    }

    const LOCAL: [(&str, &str); 3] = [("host", "localhost:7337"), ("origin", "http://localhost:7337"), ("content-type", "application/json")];

    #[tokio::test]
    async fn same_origin_ui_calls_work() {
        let env = Env::new();
        let app = app(env.hub.clone(), Arc::new(Guard::new("127.0.0.1", 7337, &[], None)));
        assert_eq!(status(&app, req(Method::GET, "/api/health", &[("host", "127.0.0.1:7337")], "")).await, StatusCode::OK);
        let res = app.clone().oneshot(req(Method::POST, "/api/get_overview", &LOCAL, "{}")).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(v.get("last_sync").is_some() || v.get("stats").is_some(), "{v}");
        // the embedded UI is served
        assert_eq!(status(&app, req(Method::GET, "/", &[("host", "localhost:7337")], "")).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn cross_site_post_is_blocked_and_changes_nothing() {
        let env = Env::new();
        let app = app(env.hub.clone(), Arc::new(Guard::new("127.0.0.1", 7337, &[], None)));
        let before = env.hub.config().sources.len();
        let evil = [("host", "127.0.0.1:7337"), ("origin", "https://evil.example"), ("content-type", "application/json")];
        let body = r#"{"type":"generic","name":"pwn","root":"C:/"}"#;
        assert_eq!(status(&app, req(Method::POST, "/api/add_source", &evil, body)).await, StatusCode::FORBIDDEN);
        // a "simple" cross-site form post (no preflight) is refused by content type as well
        let form = [("host", "127.0.0.1:7337"), ("content-type", "text/plain")];
        assert_eq!(status(&app, req(Method::POST, "/api/add_source", &form, body)).await, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        // DNS rebinding: attacker hostname resolving to 127.0.0.1
        let rebind = [("host", "attacker.example:7337"), ("content-type", "application/json")];
        assert_eq!(status(&app, req(Method::POST, "/api/add_source", &rebind, body)).await, StatusCode::FORBIDDEN);
        assert_eq!(env.hub.config().sources.len(), before, "a blocked request must not reach the handler");
    }

    #[tokio::test]
    async fn token_mode_gates_everything_but_health() {
        let env = Env::new();
        let guard = Arc::new(Guard::new("0.0.0.0", 7337, &[], Some("tok123".into())));
        let app = app(env.hub.clone(), guard);
        let host = ("host", "192.168.1.5:7337");
        assert_eq!(status(&app, req(Method::GET, "/api/health", &[host], "")).await, StatusCode::OK);
        assert_eq!(status(&app, req(Method::GET, "/", &[host], "")).await, StatusCode::UNAUTHORIZED);
        assert_eq!(status(&app, req(Method::POST, "/api/get_overview", &[host, ("content-type", "application/json")], "{}")).await, StatusCode::UNAUTHORIZED);

        // opening the printed link sets an HttpOnly cookie and redirects to a clean URL
        let res = app.clone().oneshot(req(Method::GET, "/?token=tok123", &[host], "")).await.unwrap();
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        assert_eq!(res.headers()[header::LOCATION], "/");
        let cookie = res.headers()[header::SET_COOKIE].to_str().unwrap().to_string();
        assert!(cookie.contains("memhub_token=tok123") && cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"), "{cookie}");

        let c = cookie.split(';').next().unwrap();
        assert_eq!(status(&app, req(Method::GET, "/", &[host, ("cookie", c)], "")).await, StatusCode::OK);
        assert_eq!(status(&app, req(Method::POST, "/api/get_overview", &[host, ("cookie", c), ("content-type", "application/json")], "{}")).await, StatusCode::OK);
        assert_eq!(status(&app, req(Method::POST, "/api/get_overview", &[host, ("authorization", "Bearer tok123"), ("content-type", "application/json")], "{}")).await, StatusCode::OK);
    }
}
