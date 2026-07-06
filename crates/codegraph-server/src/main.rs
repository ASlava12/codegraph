use anyhow::{Context, Result};
use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use clap::Parser;
use codegraph_core::CodeGraph;
use codegraph_indexer::{IndexOptions, scan_project};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use tokio::net::TcpListener;

#[derive(Debug, Parser)]
#[command(name = "codegraph-server")]
#[command(about = "Serve the CodeGraph API and web interface")]
struct Args {
    /// Project root exposed to the scanner.
    #[arg(long, default_value = ".")]
    root: PathBuf,

    /// HTTP bind host.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// HTTP bind port.
    #[arg(long, default_value_t = 3765)]
    port: u16,

    /// Include hidden files and directories.
    #[arg(long)]
    include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    include_ignored: bool,

    /// Allow scanning paths outside the configured root.
    #[arg(long)]
    allow_any_path: bool,
}

#[derive(Clone)]
struct AppState {
    root: PathBuf,
    options: IndexOptions,
    allow_any_path: bool,
}

#[derive(Debug, Deserialize)]
struct ScanQuery {
    path: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    root: String,
}

#[derive(Debug, Serialize)]
struct ScanResponse {
    root: String,
    graph: CodeGraph,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let root = args
        .root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize root {}", args.root.display()))?;
    let bind_addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .with_context(|| format!("invalid bind address {}:{}", args.host, args.port))?;

    let state = AppState {
        root,
        options: IndexOptions {
            include_hidden: args.include_hidden,
            include_ignored: args.include_ignored,
            ..IndexOptions::default()
        },
        allow_any_path: args.allow_any_path,
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/app.js", get(app_js))
        .route("/styles.css", get(styles_css))
        .route("/api/health", get(health))
        .route("/api/scan", get(scan))
        .fallback(not_found)
        .with_state(state);

    let listener = TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind {bind_addr}"))?;
    println!("CodeGraph listening on http://{bind_addr}");
    axum::serve(listener, app).await.context("server failed")?;
    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../../codegraph-web/static/index.html"))
}

async fn app_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        include_str!("../../codegraph-web/static/app.js"),
    )
}

async fn styles_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../../codegraph-web/static/styles.css"),
    )
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        root: state.root.display().to_string(),
    })
}

async fn scan(
    State(state): State<AppState>,
    Query(query): Query<ScanQuery>,
) -> Result<Json<ScanResponse>, ApiError> {
    let root = resolve_scan_root(&state, query.path.as_deref())?;
    let options = state.options.clone();
    let root_label = root.display().to_string();
    let graph = tokio::task::spawn_blocking(move || scan_project(root, &options))
        .await
        .map_err(|error| ApiError::internal(format!("scanner task failed: {error}")))?
        .map_err(|error| ApiError::internal(error.to_string()))?;

    Ok(Json(ScanResponse {
        root: root_label,
        graph,
    }))
}

fn resolve_scan_root(state: &AppState, requested: Option<&Path>) -> Result<PathBuf, ApiError> {
    let candidate = requested
        .map(|path| {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                state.root.join(path)
            }
        })
        .unwrap_or_else(|| state.root.clone());

    let canonical = candidate
        .canonicalize()
        .map_err(|error| ApiError::bad_request(format!("invalid path: {error}")))?;

    if !state.allow_any_path && !canonical.starts_with(&state.root) {
        return Err(ApiError::bad_request(
            "path is outside the configured root; restart with --allow-any-path to permit it",
        ));
    }

    Ok(canonical)
}

async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorBody {
            error: "not found".to_string(),
        }),
    )
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}
