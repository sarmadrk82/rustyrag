use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::stream::{self, Stream, StreamExt};
use rustyrag_adapters::build_rag_service;
use rustyrag_config::load_rag_config;
use rustyrag_core::RetrievedChunk;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::Path;
use tracing::info;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    pub query: String,
}

#[derive(Debug, Serialize)]
pub struct SourceCitation {
    pub title: String,
    pub source_uri: String,
    pub chunk_index: usize,
    pub score: f32,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct RetrieveResponse {
    pub chunks: Vec<SourceCitation>,
}

#[derive(Debug, Serialize)]
pub struct QueryResponse {
    pub answer: String,
    pub sources: Vec<SourceCitation>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

pub async fn serve(config_path: impl AsRef<Path>, bind: &str) -> anyhow::Result<()> {
    let config = load_rag_config(config_path.as_ref())?;
    let rag = build_rag_service(config)?;
    let state = AppState::new(rag);

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/retrieve", post(retrieve))
        .route("/v1/query", post(query))
        .route("/v1/query/stream", post(query_stream))
        .with_state(state);

    let addr: SocketAddr = bind.parse()?;
    info!(%addr, "rustyrag query api listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn retrieve(
    State(state): State<AppState>,
    Json(body): Json<QueryRequest>,
) -> Result<Json<RetrieveResponse>, (StatusCode, String)> {
    let chunks = state
        .rag
        .retrieve(&body.query)
        .await
        .map_err(internal_error)?;

    Ok(Json(RetrieveResponse {
        chunks: chunks.into_iter().map(to_citation).collect(),
    }))
}

async fn query(
    State(state): State<AppState>,
    Json(body): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, (StatusCode, String)> {
    let (answer, chunks) = state
        .rag
        .query(&body.query)
        .await
        .map_err(internal_error)?;

    Ok(Json(QueryResponse {
        answer,
        sources: chunks.into_iter().map(to_citation).collect(),
    }))
}

async fn query_stream(
    State(state): State<AppState>,
    Json(body): Json<QueryRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    let (token_stream, chunks) = state
        .rag
        .query_stream(&body.query)
        .await
        .map_err(internal_error)?;

    let sources_json = serde_json::to_string(
        &chunks
            .into_iter()
            .map(to_citation)
            .collect::<Vec<SourceCitation>>(),
    )
    .unwrap_or_else(|_| "[]".into());

    let sources_event = stream::once(async move {
        Ok(Event::default()
            .event("sources")
            .data(sources_json))
    });

    let token_events = token_stream.map(|result| {
        Ok(match result {
            Ok(token) => Event::default().event("token").data(token),
            Err(err) => Event::default().event("error").data(err.to_string()),
        })
    });

    let done_event = stream::once(async move {
        Ok(Event::default().event("done").data(""))
    });

    let combined = sources_event
        .chain(token_events)
        .chain(done_event);

    Ok(Sse::new(combined).keep_alive(KeepAlive::default()))
}

fn to_citation(chunk: RetrievedChunk) -> SourceCitation {
    SourceCitation {
        title: chunk.title,
        source_uri: chunk.source_uri,
        chunk_index: chunk.chunk_index,
        score: chunk.score,
        content: chunk.content,
    }
}

fn internal_error(err: rustyrag_core::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
