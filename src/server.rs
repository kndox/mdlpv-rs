use crate::{
    MermaidMode,
    session::{ScrollInput, Session, SessionEvent, SessionInput, SharedSessions, new_store},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{
        Html, IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use futures_util::StreamExt;
use serde::Serialize;
use std::{
    borrow::Cow,
    convert::Infallible,
    net::IpAddr,
    path::{Component, Path as FsPath, PathBuf},
    sync::Arc,
};
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

const MAX_JSON_BODY_BYTES: usize = 8 * 1024 * 1024;
const CONTENT_SECURITY_POLICY: &str = concat!(
    "default-src 'self'; ",
    "script-src 'self'; ",
    "style-src 'self' 'unsafe-inline'; ",
    "img-src 'self' http: https: data:; ",
    "font-src 'self'; ",
    "connect-src 'self'; ",
    "object-src 'none'; ",
    "base-uri 'self'; ",
    "frame-ancestors 'none'"
);
// Local images may be SVG documents; never let them run scripts if opened directly.
const IMAGE_CONTENT_SECURITY_POLICY: &str =
    "default-src 'none'; style-src 'unsafe-inline'; sandbox";
const IMAGE_EXTENSIONS: [&str; 15] = [
    "apng", "avif", "bmp", "gif", "ico", "jfif", "jpeg", "jpg", "pjp", "pjpeg", "png", "svg",
    "tif", "tiff", "webp",
];

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub mermaid_mode: MermaidMode,
    /// Secret required by the session mutation API; printed only to the parent process.
    pub token: String,
}

impl AppConfig {
    fn origin(&self) -> String {
        format!("http://{}:{}", url_host(&self.host), self.port)
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub sessions: SharedSessions,
    render_slots: Arc<tokio::sync::Semaphore>,
    assets_root: PathBuf,
    /// `None` accepts any Host, for wildcard binds reached through arbitrary addresses.
    allowed_hosts: Option<Arc<[String]>>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self {
            allowed_hosts: allowed_hosts(&config.host, config.port),
            config,
            sessions: new_store(),
            render_slots: Arc::new(tokio::sync::Semaphore::new(2)),
            assets_root: select_assets_root(
                std::env::current_exe().ok().as_deref(),
                FsPath::new(env!("CARGO_MANIFEST_DIR")),
            ),
        }
    }
}

pub fn app(state: AppState) -> Router {
    let session_api = Router::new()
        .route("/api/session", post(create_session))
        .route(
            "/api/session/{session_id}",
            post(update_session).delete(delete_session),
        )
        .route("/api/session/{session_id}/scroll", post(scroll_session))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_token));

    Router::new()
        .route("/health", get(health))
        .merge(session_api)
        .route("/api/rendered/{session_id}", get(rendered_session))
        .route("/api/export/{session_id}", get(export_session))
        .route("/api/image/{session_id}/{*path}", get(session_image))
        .route("/events/{session_id}", get(session_events))
        .route("/view/{session_id}", get(view_session))
        .route("/assets/viewer.js", get(viewer_js))
        .route("/assets/style.css", get(style_css))
        .route("/assets/export.js", get(export_js))
        .route("/assets/katex/{*path}", get(katex_asset))
        .layer(DefaultBodyLimit::max(MAX_JSON_BODY_BYTES))
        .layer(middleware::from_fn_with_state(state.clone(), check_host))
        .layer(middleware::map_response(add_security_headers))
        .with_state(state)
}

/// Ends every viewer stream so graceful shutdown does not wait on open SSE connections.
pub async fn close_all_sessions(state: &AppState) {
    for (_, session) in state.sessions.write().await.drain() {
        session.close();
    }
}

fn url_host(host: &str) -> Cow<'_, str> {
    if host.contains(':') && !host.starts_with('[') {
        Cow::Owned(format!("[{host}]"))
    } else {
        Cow::Borrowed(host)
    }
}

fn allowed_hosts(host: &str, port: u16) -> Option<Arc<[String]>> {
    let ip = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<IpAddr>()
        .ok();
    if ip.is_some_and(|ip| ip.is_unspecified()) {
        return None;
    }
    let mut names = vec![url_host(host).to_ascii_lowercase()];
    if host.eq_ignore_ascii_case("localhost") || ip.is_some_and(|ip| ip.is_loopback()) {
        names.extend(["localhost", "127.0.0.1", "[::1]"].map(String::from));
    }
    let mut hosts: Vec<String> = names.iter().map(|name| format!("{name}:{port}")).collect();
    // Browsers omit the default port from the Host header.
    if port == 80 {
        hosts.extend(names);
    }
    Some(hosts.into())
}

// Rejecting unexpected Host headers blocks DNS rebinding from web pages.
async fn check_host(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if let Some(allowed) = &state.allowed_hosts {
        let host = request
            .headers()
            .get(header::HOST)
            .and_then(|value| value.to_str().ok())
            .map(str::to_ascii_lowercase);
        if !host.is_some_and(|host| allowed.contains(&host)) {
            return Err(ApiError::BadHost);
        }
    }
    Ok(next.run(request).await)
}

async fn require_token(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if !has_valid_token(request.headers(), &state.config.token) {
        return Err(ApiError::Unauthorized);
    }
    Ok(next.run(request).await)
}

fn has_valid_token(headers: &HeaderMap, token: &str) -> bool {
    let Some(provided) = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
    else {
        return false;
    };
    // Constant-time comparison over equal-length inputs.
    provided.len() == token.len()
        && provided
            .bytes()
            .zip(token.bytes())
            .fold(0, |diff, (a, b)| diff | (a ^ b))
            == 0
}

async fn add_security_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers
        .entry(header::CONTENT_SECURITY_POLICY)
        .or_insert(HeaderValue::from_static(CONTENT_SECURITY_POLICY));
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    response
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

// Keep the permit in the worker: cancelling an HTTP request must not release
// capacity while its synchronous rendering job is still running.
async fn run_render_job<T: Send + 'static>(
    state: &AppState,
    job: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ApiError> {
    let permit = state
        .render_slots
        .clone()
        .acquire_owned()
        .await
        .expect("render semaphore stays open");
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        job()
    })
    .await
    .map_err(ApiError::RenderTask)
}

async fn create_session(
    State(state): State<AppState>,
    Json(input): Json<SessionInput>,
) -> Result<Json<CreateSessionResponse>, ApiError> {
    let enabled = matches!(state.config.mermaid_mode, MermaidMode::Merman);
    let session = run_render_job(&state, move || Session::new(input, enabled)).await?;
    let id = session.id;
    let view_url = format!("{}/view/{id}", state.config.origin());

    state.sessions.write().await.insert(id, session);

    Ok(Json(CreateSessionResponse {
        session_id: id,
        view_url,
    }))
}

async fn update_session(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(input): Json<SessionInput>,
) -> Result<Json<UpdateSessionResponse>, ApiError> {
    let update_lock = {
        let sessions = state.sessions.read().await;
        sessions
            .get(&session_id)
            .ok_or(ApiError::SessionNotFound)?
            .update_lock
            .clone()
    };
    let _guard = update_lock.lock().await;
    let enabled = matches!(state.config.mermaid_mode, MermaidMode::Merman);
    let (input, rendered) = run_render_job(&state, move || {
        let rendered = crate::markdown::render_markdown(&input.content, Some(session_id), enabled);
        (input, rendered)
    })
    .await?;
    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(ApiError::SessionNotFound)?;
    let event = session.update(input, rendered);

    Ok(Json(UpdateSessionResponse {
        ok: true,
        revision: event.revision,
    }))
}

async fn scroll_session(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(input): Json<ScrollInput>,
) -> Result<Json<ScrollResponse>, ApiError> {
    let sessions = state.sessions.read().await;
    let session = sessions.get(&session_id).ok_or(ApiError::SessionNotFound)?;
    let event = session.scroll(input);

    Ok(Json(ScrollResponse {
        ok: true,
        line: event.line,
    }))
}

async fn delete_session(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<OkResponse>, ApiError> {
    let mut sessions = state.sessions.write().await;
    let session = sessions
        .remove(&session_id)
        .ok_or(ApiError::SessionNotFound)?;
    session.close();

    Ok(Json(OkResponse { ok: true }))
}

async fn rendered_session(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<RenderedSessionResponse>, ApiError> {
    let sessions = state.sessions.read().await;
    let session = sessions.get(&session_id).ok_or(ApiError::SessionNotFound)?;

    Ok(Json(RenderedSessionResponse {
        session_id,
        revision: session.revision,
        title: session.title.clone(),
        html: session.rendered.html.clone(),
        has_mermaid: session.rendered.has_mermaid,
        has_math: session.rendered.has_math,
    }))
}

async fn export_session(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<Html<String>, ApiError> {
    let sessions = state.sessions.read().await;
    let session = sessions.get(&session_id).ok_or(ApiError::SessionNotFound)?;

    Ok(Html(export_html(&state.config, session)))
}

async fn session_image(
    State(state): State<AppState>,
    Path((session_id, path)): Path<(Uuid, String)>,
) -> Result<Response, ApiError> {
    let sessions = state.sessions.read().await;
    let session = sessions.get(&session_id).ok_or(ApiError::SessionNotFound)?;
    let image_path = resolve_session_file(&session.path, &path)?;
    drop(sessions);
    if !is_image_path(&image_path) {
        return Err(ApiError::Forbidden);
    }

    match tokio::fs::read(&image_path).await {
        Ok(bytes) => Ok((
            [
                (header::CONTENT_TYPE, content_type_for_path(&image_path)),
                (
                    header::CONTENT_SECURITY_POLICY,
                    IMAGE_CONTENT_SECURITY_POLICY,
                ),
            ],
            bytes,
        )
            .into_response()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(ApiError::FileNotFound),
        Err(err) => Err(ApiError::Io(err)),
    }
}

async fn session_events(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let receiver = {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&session_id).ok_or(ApiError::SessionNotFound)?;
        session.updates.subscribe()
    };

    let stream = BroadcastStream::new(receiver).filter_map(|event| async move {
        let Ok(session_event) = event else {
            return None;
        };
        let event = match session_event {
            SessionEvent::Update(update) => Event::default()
                .event("update")
                .json_data(update)
                .expect("serializable update event"),
            SessionEvent::Scroll(scroll) => Event::default()
                .event("scroll")
                .json_data(scroll)
                .expect("serializable scroll event"),
            SessionEvent::Close(close) => Event::default()
                .event("close")
                .json_data(close)
                .expect("serializable close event"),
        };
        Some(Ok(event))
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn view_session(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<Html<String>, ApiError> {
    let sessions = state.sessions.read().await;
    if !sessions.contains_key(&session_id) {
        return Err(ApiError::SessionNotFound);
    }
    drop(sessions);

    Ok(Html(viewer_html(session_id)))
}

async fn viewer_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../assets/viewer.js"),
    )
}

async fn style_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../assets/style.css"),
    )
}

async fn export_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../assets/export.js"),
    )
}

async fn katex_asset(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Result<Response, ApiError> {
    let path = resolve_asset_file(&state.assets_root, "katex", &path)?;

    match tokio::fs::read(&path).await {
        Ok(bytes) => Ok((
            [(header::CONTENT_TYPE, content_type_for_path(&path))],
            bytes,
        )
            .into_response()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(ApiError::FileNotFound),
        Err(err) => Err(ApiError::Io(err)),
    }
}

fn viewer_html(session_id: Uuid) -> String {
    // A data attribute keeps the viewer free of inline scripts under the CSP.
    include_str!("../assets/viewer.html").replace("__MDLIVE_SESSION_ID__", &session_id.to_string())
}

fn export_html(config: &AppConfig, session: &Session) -> String {
    let origin = config.origin();
    let mut head = format!(
        r#"<base href="{origin}/">
<style>
{}
</style>
"#,
        include_str!("../assets/style.css")
    );

    if session.rendered.has_math {
        head.push_str(r#"<link rel="stylesheet" href="/assets/katex/katex.min.css">"#);
        head.push('\n');
        head.push_str(r#"<script defer src="/assets/katex/katex.min.js"></script>"#);
        head.push('\n');
        head.push_str(r#"<script defer src="/assets/export.js"></script>"#);
        head.push('\n');
    }
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{}</title>
  {}
</head>
<body>
  <main class="markdown-body">
{}
  </main>
</body>
</html>
"#,
        escape_html_text(&session.title),
        head,
        session.rendered.html,
    )
}

fn resolve_session_file(markdown_path: &str, path: &str) -> Result<PathBuf, ApiError> {
    let base = FsPath::new(markdown_path)
        .parent()
        .unwrap_or_else(|| FsPath::new("."))
        .canonicalize()
        .map_err(ApiError::Io)?;
    resolve_file_under_base(base, path)
}

fn select_assets_root(executable: Option<&FsPath>, manifest_dir: &FsPath) -> PathBuf {
    if let Some(root) = executable
        .and_then(FsPath::parent)
        .map(|parent| parent.join("assets"))
        .filter(|root| root.is_dir())
    {
        return root;
    }
    manifest_dir.join("assets")
}

fn resolve_asset_file(
    assets_root: &FsPath,
    asset_dir: &str,
    path: &str,
) -> Result<PathBuf, ApiError> {
    let base = assets_root
        .join(asset_dir)
        .canonicalize()
        .map_err(ApiError::Io)?;
    resolve_file_under_base(base, path)
}

// `path` is already percent-decoded by the router; decoding again would alter
// file names that contain `%`.
fn resolve_file_under_base(base: PathBuf, path: &str) -> Result<PathBuf, ApiError> {
    let relative_path = safe_relative_path(path)?;
    let target = canonicalize_existing(base.join(relative_path))?;

    if !target.starts_with(base) {
        return Err(ApiError::Forbidden);
    }

    Ok(target)
}

fn safe_relative_path(path: &str) -> Result<PathBuf, ApiError> {
    let path = FsPath::new(path);
    if path.is_absolute() {
        return Err(ApiError::Forbidden);
    }

    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => safe.push(part),
            Component::CurDir => {}
            _ => return Err(ApiError::Forbidden),
        }
    }

    if safe.as_os_str().is_empty() {
        return Err(ApiError::Forbidden);
    }
    Ok(safe)
}

fn canonicalize_existing(path: PathBuf) -> Result<PathBuf, ApiError> {
    path.canonicalize().map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            ApiError::FileNotFound
        } else {
            ApiError::Io(err)
        }
    })
}

fn is_image_path(path: &FsPath) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| IMAGE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

fn content_type_for_path(path: &FsPath) -> &'static str {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "apng" => "image/apng",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "css" => "text/css; charset=utf-8",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "jfif" | "jpeg" | "jpg" | "pjp" | "pjpeg" => "image/jpeg",
        "js" => "text/javascript; charset=utf-8",
        "otf" => "font/otf",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "tif" | "tiff" => "image/tiff",
        "ttf" => "font/ttf",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn escape_html_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
}

#[derive(Debug, Serialize)]
struct OkResponse {
    ok: bool,
}

#[derive(Debug, Serialize)]
struct CreateSessionResponse {
    session_id: Uuid,
    view_url: String,
}

#[derive(Debug, Serialize)]
struct UpdateSessionResponse {
    ok: bool,
    revision: u64,
}

#[derive(Debug, Serialize)]
struct ScrollResponse {
    ok: bool,
    line: u64,
}

#[derive(Debug, Serialize)]
struct RenderedSessionResponse {
    session_id: Uuid,
    revision: u64,
    title: String,
    html: String,
    has_mermaid: bool,
    has_math: bool,
}

#[derive(Debug, thiserror::Error)]
enum ApiError {
    #[error("rendering task failed: {0}")]
    RenderTask(tokio::task::JoinError),
    #[error("session not found")]
    SessionNotFound,
    #[error("file not found")]
    FileNotFound,
    #[error("forbidden path")]
    Forbidden,
    #[error("unauthorized")]
    Unauthorized,
    #[error("unexpected host")]
    BadHost,
    #[error("io error: {0}")]
    Io(std::io::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            ApiError::SessionNotFound => StatusCode::NOT_FOUND,
            ApiError::FileNotFound => StatusCode::NOT_FOUND,
            ApiError::Forbidden => StatusCode::FORBIDDEN,
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::BadHost => StatusCode::FORBIDDEN,
            ApiError::Io(_) | ApiError::RenderTask(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, self.to_string()).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Method, Request},
    };
    use serde_json::{Value, json};
    use std::{fs, path::PathBuf};
    use tower::ServiceExt;

    const TEST_HOST: &str = "127.0.0.1:12345";
    const TEST_TOKEN: &str = "test-token";

    fn test_config(host: &str) -> AppConfig {
        AppConfig {
            host: host.to_string(),
            port: 12345,
            mermaid_mode: MermaidMode::Merman,
            token: TEST_TOKEN.to_string(),
        }
    }

    fn test_state() -> AppState {
        AppState::new(test_config("127.0.0.1"))
    }

    fn test_app() -> Router {
        app(test_state())
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let response = test_app()
            .oneshot(
                request_builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn responses_include_browser_security_headers() {
        let response = test_app()
            .oneshot(
                request_builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_SECURITY_POLICY)
                .unwrap(),
            CONTENT_SECURITY_POLICY
        );
        assert_eq!(
            response
                .headers()
                .get(HeaderName::from_static("referrer-policy"))
                .unwrap(),
            "no-referrer"
        );
        assert_eq!(
            response
                .headers()
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .unwrap(),
            "nosniff"
        );
        assert_eq!(
            response.headers().get(header::X_FRAME_OPTIONS).unwrap(),
            "DENY"
        );
    }

    #[tokio::test]
    async fn rejects_oversized_session_payloads() {
        let oversized_content = "x".repeat(MAX_JSON_BODY_BYTES + 1);
        let response = test_app()
            .oneshot(json_request(
                Method::POST,
                "/api/session",
                json!({
                    "path": "/tmp/a.md",
                    "title": "a.md",
                    "content": oversized_content
                }),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn creates_updates_and_renders_session() {
        let app = test_app();
        let create_response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/session",
                json!({
                    "path": "/tmp/a.md",
                    "title": "a.md",
                    "content": "# A\n\n$1+1$\n\n```mermaid\ngraph TD\nA-->B\n```"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::OK);
        let create_body = response_json(create_response).await;
        let session_id = create_body["session_id"].as_str().unwrap();

        let rendered_response = app
            .clone()
            .oneshot(
                request_builder()
                    .uri(format!("/api/rendered/{session_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let rendered_body = response_json(rendered_response).await;
        assert_eq!(rendered_body["revision"], 1);
        assert_eq!(rendered_body["has_mermaid"], true);
        assert_eq!(rendered_body["has_math"], true);
        assert!(
            rendered_body["html"]
                .as_str()
                .unwrap()
                .contains("data:image/svg+xml;base64,")
        );

        let update_response = app
            .oneshot(json_request(
                Method::POST,
                &format!("/api/session/{session_id}"),
                json!({
                    "path": "/tmp/a.md",
                    "title": "a.md",
                    "content": "# B"
                }),
            ))
            .await
            .unwrap();
        let update_body = response_json(update_response).await;
        assert_eq!(update_body["revision"], 2);
    }

    #[tokio::test]
    async fn disabled_mermaid_applies_to_creation_update_and_export() {
        let mut state = test_state();
        state.config.mermaid_mode = MermaidMode::None;
        let input = SessionInput {
            path: "/tmp/a.md".into(),
            title: "a.md".into(),
            content: "```mermaid\ngraph TD\nA-->B\n```".into(),
        };
        let Json(created) = create_session(State(state.clone()), Json(input.clone()))
            .await
            .unwrap();
        let _ = update_session(State(state.clone()), Path(created.session_id), Json(input))
            .await
            .unwrap();
        let sessions = state.sessions.read().await;
        let session = &sessions[&created.session_id];
        assert!(session.rendered.has_mermaid);
        let html = export_html(&state.config, session);
        assert!(html.contains("language-mermaid"));
        assert!(!html.contains("data:image/svg+xml;base64,"));
    }

    #[tokio::test]
    async fn updates_are_serialized_without_holding_the_session_map_lock() {
        let state = test_state();
        let input = SessionInput {
            path: "/tmp/a.md".into(),
            title: "a.md".into(),
            content: "# A".into(),
        };
        let Json(created) = create_session(State(state.clone()), Json(input.clone()))
            .await
            .unwrap();
        let id = created.session_id;
        let update_lock = state.sessions.read().await[&id].update_lock.clone();
        let permits = state
            .render_slots
            .clone()
            .acquire_many_owned(2)
            .await
            .unwrap();
        let first_state = state.clone();
        let mut first_input = input.clone();
        first_input.content = "# First".into();
        let first = tokio::spawn(async move {
            update_session(State(first_state), Path(id), Json(first_input)).await
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while update_lock.try_lock().is_ok() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        // A queued render must not prevent reads or writes for other sessions.
        let sessions = state
            .sessions
            .try_write()
            .expect("render must not hold the map lock");
        assert_eq!(sessions[&id].revision, 1);
        drop(sessions);
        let second_state = state.clone();
        let mut second_input = input;
        second_input.content = "# Second".into();
        let second = tokio::spawn(async move {
            update_session(State(second_state), Path(id), Json(second_input)).await
        });
        drop(permits);
        assert_eq!(first.await.unwrap().unwrap().0.revision, 2);
        assert_eq!(second.await.unwrap().unwrap().0.revision, 3);
        let sessions = state.sessions.read().await;
        assert_eq!(sessions[&id].content, "# Second");
        assert!(sessions[&id].rendered.html.contains("<h1>Second</h1>"));
    }

    #[tokio::test]
    async fn cancelled_request_keeps_worker_capacity_until_render_finishes() {
        let state = test_state();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (finish_tx, finish_rx) = std::sync::mpsc::channel();
        let worker_state = state.clone();
        let task = tokio::spawn(async move {
            run_render_job(&worker_state, move || {
                started_tx.send(()).unwrap();
                finish_rx.recv().unwrap();
            })
            .await
        });
        started_rx.await.unwrap();
        assert_eq!(state.render_slots.available_permits(), 1);
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_eq!(state.render_slots.available_permits(), 1);
        finish_tx.send(()).unwrap();
        let _permits = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            state.render_slots.clone().acquire_many_owned(2),
        )
        .await
        .unwrap()
        .unwrap();
    }

    #[tokio::test]
    async fn accepts_scroll_for_session() {
        let app = test_app();
        let create_response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/session",
                json!({
                    "path": "/tmp/a.md",
                    "title": "a.md",
                    "content": "# A"
                }),
            ))
            .await
            .unwrap();
        let create_body = response_json(create_response).await;
        let session_id = create_body["session_id"].as_str().unwrap();

        let response = app
            .oneshot(json_request(
                Method::POST,
                &format!("/api/session/{session_id}/scroll"),
                json!({ "line": 7 }),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["ok"], true);
        assert_eq!(body["line"], 7);
    }

    #[tokio::test]
    async fn scroll_for_unknown_session_returns_404() {
        let response = test_app()
            .oneshot(json_request(
                Method::POST,
                &format!("/api/session/{}/scroll", Uuid::new_v4()),
                json!({ "line": 7 }),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn exports_standalone_html_for_session() {
        let app = test_app();
        let create_response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/session",
                json!({
                    "path": "/tmp/a.md",
                    "title": "a.md",
                    "content": "# Export\n\n$1+1$\n\n```mermaid\ngraph TD\nA-->B\n```"
                }),
            ))
            .await
            .unwrap();
        let create_body = response_json(create_response).await;
        let session_id = create_body["session_id"].as_str().unwrap();

        let response = app
            .oneshot(
                request_builder()
                    .uri(format!("/api/export/{session_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let html = response_text(response).await;
        assert!(html.contains("<!doctype html>"));
        assert!(html.contains("/assets/katex/katex.min.css"));
        assert!(html.contains("<h1>Export</h1>"));
        assert!(html.contains("data:image/svg+xml;base64,"));
        assert!(!html.contains("mermaid.min.js"));
    }

    #[tokio::test]
    async fn delete_removes_session_from_api_surface() {
        let state = test_state();
        let app = app(state.clone());
        let create_response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/session",
                json!({
                    "path": "/tmp/a.md",
                    "title": "a.md",
                    "content": "# A"
                }),
            ))
            .await
            .unwrap();
        let create_body = response_json(create_response).await;
        let session_id = create_body["session_id"].as_str().unwrap();
        let session_id = Uuid::parse_str(session_id).unwrap();
        let mut receiver = {
            let sessions = state.sessions.read().await;
            sessions.get(&session_id).unwrap().updates.subscribe()
        };

        let delete_response = app
            .clone()
            .oneshot(
                request_builder()
                    .method(Method::DELETE)
                    .uri(format!("/api/session/{session_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {TEST_TOKEN}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(delete_response.status(), StatusCode::OK);
        match receiver.try_recv().unwrap() {
            SessionEvent::Close(close) => assert_eq!(close.reason, "stopped"),
            SessionEvent::Update(_) | SessionEvent::Scroll(_) => panic!("expected close event"),
        }

        for uri in [
            format!("/api/rendered/{session_id}"),
            format!("/view/{session_id}"),
            format!("/events/{session_id}"),
        ] {
            let response = app
                .clone()
                .oneshot(request_builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
    }

    #[tokio::test]
    async fn serves_katex_asset_with_content_type() {
        let response = test_app()
            .oneshot(
                request_builder()
                    .uri("/assets/katex/katex.min.css")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/css; charset=utf-8"
        );
    }

    #[test]
    fn prefers_assets_next_to_executable_over_build_directory() {
        let dir = temp_dir();
        let installed_assets = dir.join("installed/assets");
        let manifest_dir = dir.join("build");
        fs::create_dir_all(&installed_assets).unwrap();
        fs::create_dir_all(manifest_dir.join("assets")).unwrap();

        assert_eq!(
            select_assets_root(Some(&dir.join("installed/mdlpv-rs")), &manifest_dir),
            installed_assets
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn falls_back_to_build_assets_when_executable_assets_are_unavailable() {
        let dir = temp_dir();
        let manifest_dir = dir.join("build");
        fs::create_dir_all(manifest_dir.join("assets")).unwrap();
        // A file named assets is not an asset directory.
        fs::create_dir_all(dir.join("installed")).unwrap();
        fs::write(dir.join("installed/assets"), b"not a directory").unwrap();

        for executable in [
            None,
            Some(dir.join("installed/mdlpv-rs")),
            Some(dir.join("target/debug/mdlpv-rs")),
        ] {
            assert_eq!(
                select_assets_root(executable.as_deref(), &manifest_dir),
                manifest_dir.join("assets")
            );
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn serves_relocated_assets_without_build_directory() {
        let dir = temp_dir();
        let assets = dir.join("assets");
        let files = [
            (
                "katex/katex.min.js",
                "text/javascript; charset=utf-8",
                "installed katex",
            ),
            (
                "katex/katex.min.css",
                "text/css; charset=utf-8",
                "installed stylesheet",
            ),
            ("katex/fonts/test.woff2", "font/woff2", "installed font"),
        ];
        for (path, _, contents) in files {
            let path = assets.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }
        let mut state = test_state();
        state.assets_root =
            select_assets_root(Some(&dir.join("mdlpv-rs")), &dir.join("missing-build"));
        let app = app(state);

        for (path, content_type, contents) in files {
            let response = app
                .clone()
                .oneshot(
                    request_builder()
                        .uri(format!("/assets/{path}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers().get(header::CONTENT_TYPE).unwrap(),
                content_type
            );
            assert_eq!(response_text(response).await, contents);
        }
        // Do not fall back to checkout assets if a selected asset is missing.
        fs::remove_file(assets.join("katex/katex.min.js")).unwrap();
        for path in [
            "katex/katex.min.js",
            "katex/missing.js",
            "katex/fonts/missing.woff2",
        ] {
            let response = app
                .clone()
                .oneshot(
                    request_builder()
                        .uri(format!("/assets/{path}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_asset_symlinks_outside_selected_directory() {
        let dir = temp_dir();
        let assets = dir.join("assets");
        fs::create_dir_all(assets.join("katex")).unwrap();
        let outside = dir.join("outside.js");
        fs::write(&outside, b"outside").unwrap();
        std::os::unix::fs::symlink(outside, assets.join("katex/link.js")).unwrap();

        assert!(matches!(
            resolve_asset_file(&assets, "katex", "link.js"),
            Err(ApiError::Forbidden)
        ));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_asset_path_traversal() {
        assert!(matches!(
            resolve_asset_file(
                &FsPath::new(env!("CARGO_MANIFEST_DIR")).join("assets"),
                "katex",
                "../mermaid/mermaid.min.js"
            ),
            Err(ApiError::Forbidden)
        ));
    }

    #[tokio::test]
    async fn serves_session_images_inside_markdown_directory() {
        let dir = temp_dir();
        fs::create_dir_all(dir.join("images")).unwrap();
        fs::write(dir.join("images/a.png"), b"png").unwrap();

        let app = test_app();
        let create_response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/session",
                json!({
                    "path": dir.join("a.md").to_string_lossy().into_owned(),
                    "title": "a.md",
                    "content": "![a](images/a.png)"
                }),
            ))
            .await
            .unwrap();
        let create_body = response_json(create_response).await;
        let session_id = create_body["session_id"].as_str().unwrap();

        let response = app
            .oneshot(
                request_builder()
                    .uri(format!("/api/image/{session_id}/images/a.png"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "image/png"
        );
    }

    #[tokio::test]
    async fn rejects_session_image_path_traversal() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();

        let app = test_app();
        let create_response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/session",
                json!({
                    "path": dir.join("a.md").to_string_lossy().into_owned(),
                    "title": "a.md",
                    "content": "![a](../secret.png)"
                }),
            ))
            .await
            .unwrap();
        let create_body = response_json(create_response).await;
        let session_id = create_body["session_id"].as_str().unwrap();

        let response = app
            .oneshot(
                request_builder()
                    .uri(format!("/api/image/{session_id}/%2E%2E/secret.png"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn unknown_session_returns_404() {
        let response = test_app()
            .oneshot(
                request_builder()
                    .uri(format!("/api/rendered/{}", Uuid::new_v4()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    async fn create_test_session(app: &Router, path: &str, content: &str) -> String {
        let response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/session",
                json!({ "path": path, "title": "a.md", "content": content }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        response_json(response).await["session_id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    #[tokio::test]
    async fn session_api_requires_token() {
        let app = test_app();
        let session_id = create_test_session(&app, "/tmp/a.md", "# A").await;
        let body = json!({ "path": "/tmp/a.md", "title": "a.md", "content": "# B" }).to_string();
        for (method, uri, authorization) in [
            (Method::POST, "/api/session".to_string(), None),
            (
                Method::POST,
                "/api/session".to_string(),
                Some("Bearer wrong-token"),
            ),
            (Method::POST, "/api/session".to_string(), Some(TEST_TOKEN)),
            (Method::POST, format!("/api/session/{session_id}"), None),
            (
                Method::POST,
                format!("/api/session/{session_id}/scroll"),
                None,
            ),
            (Method::DELETE, format!("/api/session/{session_id}"), None),
        ] {
            let mut request = Request::builder()
                .method(method)
                .uri(&uri)
                .header(header::HOST, TEST_HOST)
                .header(header::CONTENT_TYPE, "application/json");
            if let Some(authorization) = authorization {
                request = request.header(header::AUTHORIZATION, authorization);
            }
            let response = app
                .clone()
                .oneshot(request.body(Body::from(body.clone())).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{uri}");
        }
        let rendered = response_json(
            app.oneshot(get_request(&format!("/api/rendered/{session_id}")))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(rendered["revision"], 1);
    }

    #[tokio::test]
    async fn rejects_unexpected_host_headers() {
        for host in [None, Some("evil.example:12345"), Some("127.0.0.1:1")] {
            let mut request = Request::builder().uri("/health");
            if let Some(host) = host {
                request = request.header(header::HOST, host);
            }
            let response = test_app()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{host:?}");
            assert!(
                response
                    .headers()
                    .contains_key(header::CONTENT_SECURITY_POLICY)
            );
        }
        for host in ["127.0.0.1:12345", "LOCALHOST:12345", "[::1]:12345"] {
            let response = test_app()
                .oneshot(
                    Request::builder()
                        .uri("/health")
                        .header(header::HOST, host)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{host}");
        }
    }

    #[test]
    fn host_allowlist_follows_bind_address() {
        assert!(allowed_hosts("0.0.0.0", 1).is_none());
        assert!(allowed_hosts("::", 1).is_none());
        assert_eq!(
            allowed_hosts("192.168.1.2", 1).unwrap().as_ref(),
            ["192.168.1.2:1".to_string()]
        );
        let default_port = allowed_hosts("127.0.0.1", 80).unwrap();
        assert!(default_port.contains(&"localhost".to_string()));
        assert!(default_port.contains(&"127.0.0.1:80".to_string()));
        assert!(
            allowed_hosts("::1", 1)
                .unwrap()
                .contains(&"[::1]:1".to_string())
        );
    }

    #[tokio::test]
    async fn view_url_brackets_ipv6_hosts() {
        let state = AppState::new(test_config("::1"));
        let input = SessionInput {
            path: "/tmp/a.md".into(),
            title: "a.md".into(),
            content: "# A".into(),
        };
        let Json(created) = create_session(State(state), Json(input)).await.unwrap();
        assert!(created.view_url.starts_with("http://[::1]:12345/view/"));
    }

    #[tokio::test]
    async fn viewer_has_no_inline_script() {
        let app = test_app();
        let session_id = create_test_session(&app, "/tmp/a.md", "# A").await;
        let html = response_text(
            app.oneshot(get_request(&format!("/view/{session_id}")))
                .await
                .unwrap(),
        )
        .await;
        assert!(html.contains(&format!(r#"data-session-id="{session_id}""#)));
        assert_eq!(html.matches("<script").count(), 1);
        assert!(html.contains(r#"<script src="/assets/viewer.js"></script>"#));
        assert!(
            CONTENT_SECURITY_POLICY
                .split("; ")
                .any(|directive| directive == "script-src 'self'")
        );
    }

    #[tokio::test]
    async fn export_loads_math_renderer_from_asset() {
        let app = test_app();
        let session_id = create_test_session(&app, "/tmp/a.md", "$1+1$").await;
        let html = response_text(
            app.clone()
                .oneshot(get_request(&format!("/api/export/{session_id}")))
                .await
                .unwrap(),
        )
        .await;
        assert!(html.contains(r#"<script defer src="/assets/export.js"></script>"#));
        assert!(!html.contains("<script>"));
        let response = app.oneshot(get_request("/assets/export.js")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn close_all_sessions_ends_viewer_streams() {
        let state = test_state();
        let input = SessionInput {
            path: "/tmp/a.md".into(),
            title: "a.md".into(),
            content: "# A".into(),
        };
        let Json(created) = create_session(State(state.clone()), Json(input))
            .await
            .unwrap();
        let mut receiver = state.sessions.read().await[&created.session_id]
            .updates
            .subscribe();
        close_all_sessions(&state).await;
        assert!(state.sessions.read().await.is_empty());
        assert!(matches!(receiver.recv().await, Ok(SessionEvent::Close(_))));
        assert!(receiver.recv().await.is_err(), "channel must be closed");
    }

    #[tokio::test]
    async fn session_images_reject_non_image_files_and_keep_percent_names() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("secret.txt"), b"secret").unwrap();
        fs::write(dir.join("100%.PNG"), b"png").unwrap();
        fs::write(dir.join("scan.tiff"), b"tiff").unwrap();
        fs::write(dir.join("a.svg"), b"<svg/>").unwrap();
        let app = test_app();
        let session_id =
            create_test_session(&app, &dir.join("a.md").to_string_lossy(), "# A").await;

        let response = app
            .clone()
            .oneshot(get_request(&format!("/api/image/{session_id}/secret.txt")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let response = app
            .clone()
            .oneshot(get_request(&format!("/api/image/{session_id}/100%25.PNG")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "image/png"
        );

        let response = app
            .clone()
            .oneshot(get_request(&format!("/api/image/{session_id}/scan.tiff")))
            .await
            .unwrap();
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "image/tiff"
        );

        let response = app
            .oneshot(get_request(&format!("/api/image/{session_id}/a.svg")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_SECURITY_POLICY)
                .unwrap(),
            IMAGE_CONTENT_SECURITY_POLICY
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn token_comparison_requires_exact_bearer_value() {
        let mut headers = HeaderMap::new();
        assert!(!has_valid_token(&headers, "abc"));
        for (value, expected) in [
            ("Bearer abc", true),
            ("Bearer abd", false),
            ("Bearer ab", false),
            ("bearer abc", false),
            ("abc", false),
        ] {
            headers.insert(header::AUTHORIZATION, HeaderValue::from_static(value));
            assert_eq!(has_valid_token(&headers, "abc"), expected, "{value}");
        }
    }

    fn request_builder() -> axum::http::request::Builder {
        Request::builder().header(header::HOST, TEST_HOST)
    }

    fn get_request(uri: &str) -> Request<Body> {
        request_builder().uri(uri).body(Body::empty()).unwrap()
    }

    fn json_request(method: Method, uri: &str, body: Value) -> Request<Body> {
        request_builder()
            .method(method)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {TEST_TOKEN}"))
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    async fn response_json(response: Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn response_text(response: Response) -> String {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("mdlpv-rs-test-{}", Uuid::new_v4()))
    }
}
