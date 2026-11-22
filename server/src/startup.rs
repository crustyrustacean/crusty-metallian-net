// src/startup.rs

// dependencies
use crate::configuration::{BasicAuthSettings, CorsSettings, Settings};
use crate::database::DatabaseBackend;
use crate::metadata::MetadataClient;
use crate::middleware::basic_auth;
use crate::routes::{api, frontend};
use crate::telemetry::request_id;
use crate::template::TemplateRenderer;
use crate::utils::shutdown_signal;
use actix_cors::Cors;
use actix_files::Files;
use actix_web::dev::Server;
use actix_web::http::header::{ACCEPT, CONTENT_TYPE, HeaderName, HeaderValue};
use actix_web::{App, HttpResponse, HttpServer, middleware, web, web::Data};
use std::net::TcpListener;
use std::sync::Arc;
use tracing_actix_web::TracingLogger;

const DATASTAR_REQUEST: HeaderName = HeaderName::from_static("datastar-request");

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(
        configuration: Settings,
        template_renderer: Arc<dyn TemplateRenderer>,
        database: Arc<dyn DatabaseBackend>,
        metadata: MetadataClient,
    ) -> Result<Self, anyhow::Error> {
        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );
        let listener = TcpListener::bind(address)?;
        let port = listener.local_addr()?.port();
        let server = run(
            listener,
            template_renderer,
            database,
            metadata,
            configuration.basicauth,
            configuration.application.static_dir,
            configuration.cors,
        )
        .await?;
        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        let server = self.server;
        let handle = server.handle();

        tokio::select! {
            result = server => result?,
            _ = shutdown_signal() => {
                handle.stop(true).await;
            }
        }

        tracing::info!("shutdown complete");
        Ok(())
    }
}

async fn run(
    listener: TcpListener,
    template_renderer: Arc<dyn TemplateRenderer>,
    database: Arc<dyn DatabaseBackend>,
    metadata: MetadataClient,
    basicauth: BasicAuthSettings,
    static_dir: String,
    cors_settings: CorsSettings,
) -> Result<Server, anyhow::Error> {
    let template_renderer = Data::from(template_renderer);
    let database = Data::new(database);
    let metadata = Data::new(metadata);
    let basicauth = Data::new(basicauth);
    let cors_origins = cors_settings
        .allowed_origins
        .iter()
        .map(|origin| {
            HeaderValue::from_str(origin)
                .map_err(|_| anyhow::anyhow!("invalid CORS origin in configuration: {origin:?}"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let server = HttpServer::new(move || {
        let cors_origins = cors_origins.clone();
        let cors = Cors::default()
            .allowed_methods(["GET", "HEAD"])
            .allowed_headers([CONTENT_TYPE, ACCEPT, DATASTAR_REQUEST])
            .allowed_origin_fn(move |origin, _request_head| {
                cors_origins.iter().any(|allowed| allowed == origin)
            });

        App::new()
            .wrap(middleware::from_fn(request_id))
            .wrap(TracingLogger::default())
            // CORS must wrap the entire application so preflight requests receive
            // headers before route or authentication middleware can reject them.
            .wrap(cors)
            .service(
                web::resource("/")
                    .wrap(middleware::from_fn(basic_auth))
                    .route(web::get().to(frontend::get_index_page)),
            )
            .service(
                web::resource("/stats")
                    .wrap(middleware::from_fn(basic_auth))
                    .route(web::get().to(frontend::get_stats_page)),
            )
            .service(
                web::scope("/api/v1")
                    .route("/health_check", web::get().to(api::health_check))
                    .service(
                        web::resource("/rotation")
                            .route(web::get().to(api::get_rotation_entry))
                            .route(
                                web::post()
                                    .to(api::post_rotation_entry_ds)
                                    .wrap(middleware::from_fn(basic_auth)),
                            ),
                    )
                    .route("/stats", web::get().to(api::get_artist_stats)),
            )
            .service(
                Files::new("/static", static_dir.clone())
                    .prefer_utf8(true)
                    .default_handler(web::to(HttpResponse::NotFound)),
            )
            .default_service(web::to(frontend::not_found))
            .app_data(template_renderer.clone())
            .app_data(database.clone())
            .app_data(metadata.clone())
            .app_data(basicauth.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
