use async_trait::async_trait;
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, DeletePointsBuilder, Distance, Filter, PointStruct,
    SearchPointsBuilder, UpsertPointsBuilder, Value, VectorParamsBuilder,
};
use qdrant_client::qdrant::value::Kind;
use qdrant_client::Qdrant;
use rustyrag_config::DistanceMetric;
use rustyrag_core::{ChunkRecord, Error, Result, RetrievedChunk, VectorStoreAdapter};
use std::collections::HashMap;
use std::sync::Arc;

pub struct QdrantStore {
    client: Arc<Qdrant>,
    collection: String,
    distance: DistanceMetric,
}

impl QdrantStore {
    pub fn new(url: impl Into<String>, collection: impl Into<String>, distance: DistanceMetric) -> Result<Self> {
        let client = Qdrant::from_url(&url.into())
            .build()
            .map_err(|err| adapter_err(err.to_string()))?;

        Ok(Self {
            client: Arc::new(client),
            collection: collection.into(),
            distance,
        })
    }

    fn qdrant_distance(&self) -> Distance {
        match self.distance {
            DistanceMetric::Cosine => Distance::Cosine,
            DistanceMetric::Euclid => Distance::Euclid,
            DistanceMetric::Dot => Distance::Dot,
        }
    }
}

#[async_trait]
impl VectorStoreAdapter for QdrantStore {
    async fn ensure_collection(&self, vector_size: u64) -> Result<()> {
        if self
            .client
            .collection_exists(&self.collection)
            .await
            .map_err(|err| adapter_err(err.to_string()))?
        {
            return Ok(());
        }

        self.client
            .create_collection(
                CreateCollectionBuilder::new(&self.collection).vectors_config(
                    VectorParamsBuilder::new(vector_size, self.qdrant_distance()),
                ),
            )
            .await
            .map_err(|err| adapter_err(err.to_string()))?;

        Ok(())
    }

    async fn delete_by_source_uri(&self, source_uri: &str) -> Result<()> {
        let filter = Filter::must([Condition::matches(
            "source_uri",
            source_uri.to_string(),
        )]);

        self.client
            .delete_points(DeletePointsBuilder::new(&self.collection).points(filter))
            .await
            .map_err(|err| adapter_err(err.to_string()))?;

        Ok(())
    }

    async fn upsert(&self, records: &[ChunkRecord]) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        let points: Vec<PointStruct> = records
            .iter()
            .map(|record| {
                let payload: HashMap<String, Value> = HashMap::from([
                    ("source_id".into(), Value::from(record.source_id.clone())),
                    ("source_uri".into(), Value::from(record.source_uri.clone())),
                    ("title".into(), Value::from(record.title.clone())),
                    (
                        "chunk_index".into(),
                        Value::from(record.chunk_index as i64),
                    ),
                    ("content".into(), Value::from(record.content.clone())),
                    ("content_hash".into(), Value::from(record.content_hash.clone())),
                ]);

                PointStruct::new(record.point_id, record.vector.clone(), payload)
            })
            .collect();

        self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection, points))
            .await
            .map_err(|err| adapter_err(err.to_string()))?;

        Ok(())
    }

    async fn search(&self, vector: &[f32], top_k: usize) -> Result<Vec<RetrievedChunk>> {
        let response = self
            .client
            .search_points(
                SearchPointsBuilder::new(&self.collection, vector.to_vec(), top_k as u64)
                    .with_payload(true),
            )
            .await
            .map_err(|err| adapter_err(err.to_string()))?;

        Ok(response
            .result
            .into_iter()
            .map(|point| RetrievedChunk {
                score: point.score,
                source_uri: payload_string(&point.payload, "source_uri"),
                title: payload_string(&point.payload, "title"),
                chunk_index: payload_usize(&point.payload, "chunk_index"),
                content: payload_string(&point.payload, "content"),
            })
            .collect())
    }
}

fn adapter_err(message: String) -> Error {
    Error::Adapter {
        adapter: "qdrant".into(),
        message,
    }
}

fn payload_string(payload: &HashMap<String, Value>, key: &str) -> String {
    payload
        .get(key)
        .and_then(|value| match value.kind.as_ref() {
            Some(Kind::StringValue(text)) => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn payload_usize(payload: &HashMap<String, Value>, key: &str) -> usize {
    payload
        .get(key)
        .and_then(|value| match value.kind.as_ref() {
            Some(Kind::IntegerValue(number)) => Some(*number as usize),
            _ => None,
        })
        .unwrap_or(0)
}
