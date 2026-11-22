// tests/api/helpers.rs

// dependencies
use crate::metadata_stub::MetadataStubs;
use server::configuration::{DatabaseSettings, Settings, get_configuration};
use server::database::{DatabaseBackend, SqliteRepository};
use server::metadata::MetadataClient;
use server::startup::Application;
use server::storage::{InMemoryStorageBackend, StorageBackend};
use server::telemetry::{get_subscriber, init_subscriber};
use server::template::{TemplateRenderer, tera::TeraRenderer};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};

// Ensure that the `tracing` stack is only initialised once.
static TRACING: LazyLock<()> = LazyLock::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    };
});

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(prefix: &str) -> Self {
        let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "server-test-{prefix}-{}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("Failed to create temp directory.");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[allow(dead_code)]
pub struct TestApp {
    pub address: String,
    pub port: u16,
    /// Authenticated admin client (logged in via POST /login with cookie store).
    pub api_client: reqwest::Client,
    /// Unauthenticated client (no session cookie).
    pub unauthenticated_client: reqwest::Client,
    pub metadata_stub: MetadataStubs,
    pub database_dir: tempfile::TempDir,
}

impl TestApp {
    pub async fn get(&self, path: &str) -> reqwest::Response {
        self.api_client
            .get(format!("{}{path}", self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }
}

pub async fn spawn_app() -> TestApp {
    let stub = MetadataStubs::miss().await;
    spawn_app_against(stub).await
}

pub async fn spawn_app_against(metadata_stub: MetadataStubs) -> TestApp {
    let configuration = test_configuration();
    let template_renderer: Arc<dyn TemplateRenderer> = Arc::new(
        TeraRenderer::new(&configuration.application.templates_glob)
            .expect("Failed to build the template renderer."),
    );
    spawn_app_with(configuration, template_renderer, metadata_stub).await
}

pub async fn spawn_app_with_renderer(template_renderer: Arc<dyn TemplateRenderer>) -> TestApp {
    let stub = MetadataStubs::miss().await;
    spawn_app_with(test_configuration(), template_renderer, stub).await
}

pub async fn spawn_app_with_static_dir(static_dir: impl Into<String>) -> TestApp {
    let mut configuration = test_configuration();
    configuration.application.static_dir = static_dir.into();
    let template_renderer: Arc<dyn TemplateRenderer> = Arc::new(
        TeraRenderer::new(&configuration.application.templates_glob)
            .expect("Failed to build the template renderer."),
    );
    let stub = MetadataStubs::miss().await;
    spawn_app_with(configuration, template_renderer, stub).await
}

fn test_configuration() -> Settings {
    let mut c = get_configuration().expect("Failed to read configuration.");
    c.application.port = 0;
    c
}

async fn spawn_app_with(
    configuration: Settings,
    template_renderer: Arc<dyn TemplateRenderer>,
    metadata_stub: MetadataStubs,
) -> TestApp {
    LazyLock::force(&TRACING);

    let database_dir = tempfile::tempdir().expect("Failed to create a temp directory");
    let database_settings = DatabaseSettings {
        path: database_dir.path().join("test.db").display().to_string(),
        max_connections: Some(1),
    };
    let database: Arc<dyn DatabaseBackend> = Arc::new(
        SqliteRepository::new(&database_settings)
            .await
            .expect("Failed to initialise test SQLite repository."),
    );
    let storage: Arc<dyn StorageBackend> = Arc::new(InMemoryStorageBackend::new());
    let metadata = MetadataClient::with_base_urls(
        metadata_stub.musicbrainz.uri(),
        metadata_stub.cover_art.uri(),
    );

    let admin = configuration.admin.clone();
    let application = Application::build(
        configuration,
        template_renderer,
        database,
        storage,
        metadata,
    )
    .await
    .expect("Failed to build application.");
    let application_port = application.port();
    drop(tokio::spawn(application.run_until_stopped()));

    let address = format!("http://localhost:{}", application_port);

    let api_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    // Sign in so `api_client` holds a valid admin session cookie.
    let login_response = api_client
        .post(format!("{address}/login"))
        .form(&[
            ("username", admin.username.as_str()),
            ("password", admin.password.as_str()),
        ])
        .send()
        .await
        .expect("Failed to log in test client.");
    assert_eq!(
        login_response.status(),
        reqwest::StatusCode::SEE_OTHER,
        "Test client login failed"
    );

    let unauthenticated_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    TestApp {
        address,
        port: application_port,
        api_client,
        unauthenticated_client,
        metadata_stub,
        database_dir,
    }
}

// --- Server-sent events helpers ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: Vec<String>,
}

impl SseEvent {
    pub fn dataline(&self, key: &str) -> Option<&str> {
        self.data.iter().find_map(|line| {
            line.strip_prefix(key)
                .and_then(|rest| rest.strip_prefix(' '))
        })
    }
}

pub fn parse_sse(body: &str) -> Vec<SseEvent> {
    let fresh = || SseEvent {
        event: None,
        data: Vec::new(),
    };
    let mut events = Vec::new();
    let mut event = fresh();
    for line in body.lines() {
        if line.is_empty() {
            if event.event.is_some() || !event.data.is_empty() {
                events.push(std::mem::replace(&mut event, fresh()));
            }
            continue;
        }
        if let Some(name) = line.strip_prefix("event: ") {
            assert!(
                event.event.replace(name.to_owned()).is_none(),
                "multiple event names without a blank-line separator:\n{body}"
            );
        } else if let Some(data) = line.strip_prefix("data: ") {
            event.data.push(data.to_owned());
        }
    }
    events
}

pub fn assert_sse_headers(response: &reqwest::Response) {
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .expect("SSE response must carry a content-type")
        .to_str()
        .expect("content-type must be ASCII");
    assert!(
        content_type.split(';').next().unwrap().trim() == "text/event-stream",
        "expected text/event-stream, got: {content_type}"
    );

    let cache_control = response
        .headers()
        .get(reqwest::header::CACHE_CONTROL)
        .expect("SSE response must carry cache-control")
        .to_str()
        .expect("cache-control must be ASCII");
    assert!(
        cache_control
            .split(',')
            .any(|directive| directive.trim().eq_ignore_ascii_case("no-cache")),
        "expected no-cache, got: {cache_control}"
    );
}

pub async fn sse_body(response: reqwest::Response) -> String {
    tokio::time::timeout(std::time::Duration::from_secs(10), response.text())
        .await
        .expect("the stream should end on its own within 10 seconds")
        .expect("Failed to read the stream")
}

pub fn assert_sse_framing(body: &str, expected: usize) -> Vec<SseEvent> {
    let normalized = body.replace("\r\n", "\n").replace('\r', "\n");
    assert!(
        normalized.ends_with("\n\n"),
        "the final event must be terminated by a blank line:\n{body}"
    );
    let events = parse_sse(&normalized);
    assert_eq!(
        events.len(),
        expected,
        "expected {expected} blank-line-delimited events:\n{body}"
    );
    assert!(
        events.iter().all(|e| e.event.is_some()),
        "every event must name its type:\n{body}"
    );
    events
}
