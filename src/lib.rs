use {
    crate::{
        log::create_trace_layer,
        routes::{fetch, health, index, static_files, submit},
    },
    axum::{routing::get, Router},
    color_eyre::eyre::Result,
    serde::{Deserialize, Serialize},
    std::net::SocketAddr,
    tokio::task::JoinHandle,
    tower_http::compression::CompressionLayer,
    tracing::info,
};

pub mod config;
pub mod error;
pub mod log;
pub mod routes;

pub use crate::config::Config;

/// Static files cached for 15 minutes
const STATIC_FILES_MAX_AGE: u64 = 15 * 60;

/// Starts a new instance of the contractor returning a handle
pub async fn start(config: &Config) -> Result<Handle> {
    // initialize global tracing subscriber
    let _guard = crate::log::tracing_init()?;

    config::init(config.clone()).await;

    let compression = CompressionLayer::new().br(true).deflate(true).gzip(true);

    // create router with all routes and tracing layer
    let router = Router::new()
        .route("/locations", get(fetch).post(submit))
        .route("/health", get(health))
        .route("/", get(index))
        .fallback(static_files)
        .layer(compression)
        .layer(create_trace_layer());

    // bind axum server to socket address and use router to create a service factory
    let server = axum::Server::bind(&config::get().address).serve(router.into_make_service());

    // get address server is bound to (may be different to address passed to Server::bind)
    let address = server.local_addr();

    // spawn server on new tokio task
    let handle = tokio::spawn(async { server.await.map_err(Into::into) });

    info!("contractor started on http://{}", address);

    // return handles
    Ok(Handle { address, handle })
}

/// Handle for running an instance
pub struct Handle {
    // Socket address instance is bound to
    address: SocketAddr,
    // JoinHandle for server task
    handle: JoinHandle<Result<()>>,
}

impl Handle {
    /// Gets the socket address the running instance is bound to
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Awaits on the instance's task
    pub async fn join(self) -> Result<()> {
        self.handle.await??;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocationOccupancy {
    name: String,
    occupancy: OccupancyLevel,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OccupancyLevel {
    Low,
    Medium,
    High,
}
