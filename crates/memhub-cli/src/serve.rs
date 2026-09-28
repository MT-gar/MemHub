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
use memhub_core::Hub;
use rust_embed::RustEmbed;
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(RustEmbed)]
#[folder = "../../ui/dist/"]
struct Assets;

pub fn run(hub: Arc<Hub>, host: &str, port: u16, watch: bool) -> Result<()> {
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

        let app = Router::new()
            .route("/api/health", get(|| async { Json(json!({"ok": true, "version": memhub_core::VERSION})) }))
            .route("/api/{cmd}", post(rpc))
            .fallback(static_handler)
            .with_state(hub);
        let addr = format!("{host}:{port}");
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        println!("MemHub UI: http://{}{}", if host == "0.0.0.0" { "localhost" } else { host }, format!(":{port}"));
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
