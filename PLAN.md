# RustyRAG — Product Plan

**Last updated:** June 2026  
**Project:** `/Users/radhakrishnasarma/coding/rustyrag`

This is the living plan for RustyRAG. Open it from the file tree anytime (Cmd+P → `PLAN.md`).

---

## North star

RustyRAG is a **RAG ETL framework**, not a one-off app:

1. **Flexible adapters** — sources, parsers, chunkers, embedders, vector stores (swap via config)
2. **GUI config builder** — pick options in a UI instead of hand-writing YAML
3. **Generated YAML** — GUI writes the runtime config (git-friendly, CI-friendly)
4. **Docker deliverable** — container images you run as **multiple instances** on a container platform (Kubernetes, ECS, Docker Compose at scale)

```
  GUI (Phase 4)
    ↓ generates
  YAML configs
    ↓ consumed by
  RustyRAG engine (Rust)
    ↓ deploy as
  ETL worker containers (×N)  +  Query API containers (×M)
    ↓ both talk to
  Vector store(s) — config-selected target (Qdrant today; pgVector, OpenSearch, S3 Vectors planned)
```

**YAML stays** the runtime format. **GUI is the author.** TOML for app settings is optional/later — the GUI may make TOML unnecessary for most users.

**Differentiation:** config-first pipelines + split batch ETL / query API + GUI + containers. Closer to “Airflow/dbt for RAG” than a code-first agent framework.

---

## Project identity

| Field | Value |
|---|---|
| **Name** | `rustyrag` |
| **CLI binary** | `rustyrag` |
| **Crate prefix** | `rustyrag-*` |
| **Runtime config** | YAML in `configs/pipelines/` and `configs/rag/` |
| **Secrets** | `.env` in project root (gitignored); copy from `.env.example` |
| **Collection model** | One pipeline = one logical index (collection/table/index name; backend-specific) |
| **Vector store (today)** | **Qdrant only** — sole implemented `VectorStoreAdapter` |
| **Vector store (planned)** | pgVector, AWS OpenSearch, AWS S3 Vectors — selectable via `store.adapter` |
| **Local dev vector DB** | Qdrant via `docker compose up -d` (ports 6333, 6334) |
| **Query API** | Default bind `8080`; use `--bind 0.0.0.0:8081` if port taken (e.g. Zookeeper on 8080) |

---

## What is built today (Phases 0–2, 6 partial) ✅

### Phase 0 — Scaffold
- Cargo workspace with crates: `core`, `config`, `adapters`, `etl`, `api`, `cli`
- YAML load + validate + `${ENV}` substitution
- Adapter traits (`Source`, `Parser`, `Chunker`, `Embedder`, `VectorStore`, `Llm`)
- Adapter registry: `rustyrag adapters list`

### Phase 1 — ETL
- **Source:** filesystem (`./data/docs`, glob `**/*`; `.md`, `.txt`, `.html`, `.htm`, `.pdf`)
- **Parse:** auto — plain text / markdown as-is; PDF text extraction; HTML → text
- **Chunk:** recursive (`text-splitter`) or **semantic** (embed-based topic boundaries)
- **Embed:** OpenAI or **Ollama**
- **Store:** Qdrant only (cosine, one collection per pipeline) — pgVector, OpenSearch, S3 Vectors planned (see Vector stores)
- **Idempotency:** content hash in `.rustyrag/<pipeline-name>.json`
- **Safety:** max file size, max chunks per document, embed cost estimate in dry-run
- **Checkpoint:** per-document state save for crash resume
- **CLI:** `validate`, `etl run`, `etl dry-run`

### Phase 2 — Query API
- **Endpoints:**
  - `GET /healthz`
  - `POST /v1/retrieve` — embed query + vector search (no LLM)
  - `POST /v1/query` — retrieve + LLM answer + source citations
  - `POST /v1/query/stream` — same as query, streamed via SSE
- **Retrieval:** semantic or **hybrid** (dense + BM25 rerank)
- **Generation:** OpenAI or **Ollama**
- **Config:** `configs/rag/default.yaml`
- **CLI:** `rustyrag serve --bind 0.0.0.0:8081`

### Phase 6 — Advanced adapters (partial) ✅
- Semantic chunking, hybrid search, Ollama embed/LLM
- PDF + HTML ingest via `parse.adapter: auto`
- Streaming query, file-size/chunk guards, checkpoint/resume, dry-run cost estimate
- **Not yet:** S3 source, Confluence, dedicated reranker, SQLite checkpoint store

### Demo flow (works now)

```bash
cd ~/coding/rustyrag
cp .env.example .env          # add OPENAI_API_KEY, QDRANT_URL
docker compose up -d          # starts Qdrant in Docker — not RustyRAG itself

cargo run -p rustyrag-cli -- etl dry-run configs/pipelines/docs-index.yaml
cargo run -p rustyrag-cli -- etl run configs/pipelines/docs-index.yaml
cargo run -p rustyrag-cli -- serve --bind 0.0.0.0:8081

curl http://localhost:8081/healthz
curl -X POST http://localhost:8081/v1/query \
  -H 'Content-Type: application/json' \
  -d '{"query":"How do I run the pipeline?"}'

# Stream tokens (SSE)
curl -N -X POST http://localhost:8081/v1/query/stream \
  -H 'Content-Type: application/json' \
  -d '{"query":"How do I run the pipeline?"}'
```

**Index a PDF:** drop `.pdf` files under `data/docs/` (or your pipeline `source.path`), run `etl run` — text is extracted at ingest, not at query time.

---

## Architecture

### Runtime flow

**ETL (batch):**
```
YAML pipeline config
  → read files (SourceAdapter)
  → parse (ParserAdapter)
  → chunk (ChunkerAdapter)
  → embed (EmbedderAdapter)
  → upsert vectors (VectorStoreAdapter)
  → configured vector store target
```

**Query (long-running API):**
```
YAML rag config
  → embed user question
  → search vector store (top_k)
  → build context from chunks
  → LLM generation
  → answer + citations
```

### Crate layout

```
rustyrag/
├── Cargo.toml                 # workspace
├── PLAN.md                    # this file
├── docker-compose.yaml        # Qdrant only (today)
├── .env.example
├── configs/
│   ├── pipelines/docs-index.yaml
│   └── rag/default.yaml
├── data/docs/                 # sample documents
└── crates/
    ├── rustyrag-core          # types, traits, errors
    ├── rustyrag-config        # YAML load, validate, .env
    ├── rustyrag-adapters      # filesystem, openai, qdrant, llm
    ├── rustyrag-etl           # pipeline runner, idempotency state
    ├── rustyrag-api           # axum HTTP server
    ├── rustyrag-cli           # binary: validate, etl, serve
    └── rustyrag-gui           # (Phase 4) TBD
```

### Adapter pattern

Config picks an adapter **by name**. Code implements a **trait** (interface):

| Stage | Built today | Trait |
|---|---|---|
| Source | `filesystem` | `SourceAdapter` |
| Parse | `auto` | `ParserAdapter` |
| Chunk | `recursive`, `semantic` | `ChunkerAdapter` |
| Embed | `openai`, `ollama` | `EmbedderAdapter` |
| Store | **`qdrant` only** | `VectorStoreAdapter` |
| LLM | `openai`, `ollama` | `LlmAdapter` |

Adding a new backend = new adapter impl + register name in validator/registry. GUI (Phase 4) reads the same registry.

### Vector stores (current + planned)

The `VectorStoreAdapter` trait is backend-agnostic. **Only Qdrant is implemented today.** The config contract is designed so you pick one store target per pipeline (ETL) and one per RAG config (query) — same adapter name on both sides.

| Adapter | Status | Use case | Config highlights |
|---|---|---|---|
| **`qdrant`** | ✅ Built | Local dev, self-hosted, dedicated vector DB | `url`, `collection`, `distance` |
| **`pgvector`** | ⏳ Planned | Teams already on Postgres; join vectors with relational metadata | `connection_url`, `table`, `schema`, `distance` |
| **`opensearch`** | ⏳ Planned | AWS-managed hybrid search (dense + BM25 native) | `endpoint`, `index`, `region`, auth via env/IAM |
| **`s3_vectors`** | ⏳ Planned | AWS S3 Vectors — cost-effective large-scale vector storage | `bucket`, `index_name`, `region`, IAM |

**Design rules (locked for multi-target support):**

1. **One pipeline → one store target** — ETL writes to exactly the index named in `store.*`.
2. **RAG must match ETL** — `retrieval.store.adapter` + index/collection name must point at the same backend the pipeline wrote to.
3. **Adapter-specific fields stay nested** — common fields (`adapter`, index/collection name, distance) plus a backend block or flat fields validated per adapter (GUI shows the right form per selection).
4. **No multi-write in v1** — fan-out to Qdrant *and* pgVector in one pipeline is out of scope; duplicate pipelines if you need two copies.

**Planned pipeline config (store stage):**

```yaml
store:
  adapter: qdrant          # qdrant | pgvector | opensearch | s3_vectors
  collection: docs         # logical index name (backend maps this to collection/table/index)
  distance: cosine

  # --- qdrant (today) ---
  url: "${QDRANT_URL}"

  # --- pgvector (planned) ---
  # connection_url: "${DATABASE_URL}"
  # table: chunk_vectors
  # schema: public

  # --- opensearch (planned) ---
  # endpoint: "${OPENSEARCH_ENDPOINT}"
  # region: us-east-1
  # # auth: OPENSEARCH_USERNAME/PASSWORD or AWS IAM

  # --- s3_vectors (planned) ---
  # bucket: my-vector-bucket
  # index_name: docs
  # region: us-east-1
  # # auth: AWS credentials / IAM role
```

**Planned RAG config (retrieval store):**

```yaml
retrieval:
  store_adapter: qdrant    # must match pipeline store.adapter
  collection: docs         # must match pipeline store.collection
  store_url: "${QDRANT_URL}"   # qdrant
  # connection_url: ...        # pgvector
  # endpoint: ...              # opensearch
  # bucket / index_name: ...   # s3_vectors
  embed_adapter: openai
  embed_model: text-embedding-3-small
  top_k: 5
  search_mode: semantic
```

---

## Config reference (runtime contract)

### Pipeline — `configs/pipelines/docs-index.yaml`

```yaml
version: 1
name: docs-index

source:
  adapter: filesystem
  path: ./data/docs
  glob: "**/*"

parse:
  adapter: auto

chunk:
  adapter: recursive       # future: semantic
  chunk_size: 512
  chunk_overlap: 64

embed:
  adapter: openai
  model: text-embedding-3-small
  batch_size: 64

store:
  adapter: qdrant           # only qdrant implemented today; see Vector stores section
  url: "${QDRANT_URL}"
  collection: docs          # one pipeline → one logical index
  distance: cosine

index:
  idempotency_key: content_hash
  batch_upsert_size: 100
```

### RAG — `configs/rag/default.yaml`

```yaml
version: 1
name: default-rag

retrieval:
  collection: docs          # must match pipeline store.collection
  store_url: "${QDRANT_URL}"  # qdrant-specific today; store_adapter planned for multi-backend
  embed_model: text-embedding-3-small
  top_k: 5
  search_mode: semantic     # future: hybrid
  rerank:
    enabled: false
    adapter: none

generation:
  adapter: openai
  model: gpt-4o-mini
  system_prompt: |
    Answer using only the provided context. Cite sources when possible.

context:
  max_tokens: 4000
  template: default
```

---

## Chunking (current + planned)

| Adapter | Status | Description |
|---|---|---|
| **recursive** | ✅ Built | Split by size (~512 chars), break at sentence/paragraph boundaries. Fast, no extra API cost. |
| **semantic** | ✅ Built | Embed sentences, split where topic shifts. Better coherence; costs embed calls at ingest. |
| **hybrid search** | ✅ Built | Dense + BM25 reranking via `retrieval.search_mode: hybrid`. |

---

## Docker — what it does today vs later

| Today | Phase 5 |
|---|---|
| `docker compose up -d` runs **Qdrant only** | Also run **RustyRAG ETL + API** as containers |
| RustyRAG via `cargo run` on your Mac | `docker run` / Kubernetes replicas |
| **Not** `docker build` of our app yet | Dockerfile(s) for worker + API images |

**Analogy:** Compose today = start the database. Phase 5 = package our app too, run many copies on a platform.

---

## Lessons from hands-on use

- Run **`etl dry-run`** before **`etl run`** — avoids surprise OpenAI bills
- Need **max file size / max chunks** guard (accidental multi-GB file lesson)
- Always run CLI from **project root** (`~/coding/rustyrag`)
- **`lsof -iTCP:8080`** to see port conflicts; Zookeeper used 8080 on this machine
- Inspect Qdrant: `curl http://localhost:6333/collections/docs` or dashboard at `http://localhost:6333/dashboard`
- **Vector store is Qdrant-only in code** — pgVector / OpenSearch / S3 Vectors require new `VectorStoreAdapter` impls (see roadmap)

---

## Roadmap (what’s next)

### Phase 3 — Config contract for GUI ⏳

**Goal:** GUI and engine share one source of truth.

- JSON Schema (or equivalent) for pipeline + RAG configs
- **Adapter registry as data** — name, label, fields, types, defaults
- New CLI: `rustyrag adapters list`, `rustyrag config export`
- Keep `rustyrag validate` as the gate

**Exit criteria:** A tool/GUI can list adapters and emit valid YAML without hand-editing.

---

### Phase 4 — GUI v1

**Goal:** Build pipelines visually; as flexible as YAML.

- Web UI in browser (React + Rust API, or Rust-served static — TBD)
- Screens: **Pipeline wizard** + **RAG config**
- Dropdowns from adapter registry; **Advanced** panel for power users
- Actions: validate, dry-run, export/save YAML, optional trigger ETL
- Users never required to edit raw YAML (but can)

**Exit criteria:** Create a new pipeline entirely in GUI → export YAML → ETL + query work.

---

### Phase 5 — Docker & multi-instance deployment

**Goal:** Runnable containers for production-style deployment.

- Dockerfile: ETL worker image + query API image (or one image, two commands)
- Config via mounted volume / ConfigMap (from GUI export)
- `docker compose` profile: Qdrant + worker + API
- Example Kubernetes: Deployments (replicas), Services, ConfigMaps
- Scale ETL workers for batch throughput; scale API for query load

**Exit criteria:** Deploy N workers + M API instances against shared vector store without `cargo run`.

---

### Phase 6 — Advanced adapters ✅ (partial)

**New adapters & features:**

| Feature | Config | CLI / API |
|---|---|---|
| Semantic chunking | `chunk.adapter: semantic` | Uses embed API at ingest |
| Hybrid search | `retrieval.search_mode: hybrid` | Dense + BM25 rerank |
| Ollama embed/LLM | `embed.adapter: ollama` / `generation.adapter: ollama` | Local, no API key |
| PDF / HTML parse | `parse.adapter: auto` | `.pdf`, `.html`, `.htm` files |
| Streaming query | — | `POST /v1/query/stream` (SSE) |
| File size guard | `source.max_file_size_bytes` | Skips oversized files |
| Chunk limit | `index.max_chunks_per_document` | Rejects runaway docs |
| Checkpoint/resume | `index.checkpoint: true` | Saves state per document |
| Embed cost estimate | — | `etl dry-run` prints USD estimate |
| Adapter registry | — | `rustyrag adapters list` |

**Example — hybrid RAG config snippet:**

```yaml
retrieval:
  search_mode: hybrid
  hybrid_dense_weight: 0.7   # 70% dense, 30% BM25
  embed_adapter: openai
  embed_model: text-embedding-3-small
```

**Example — Ollama pipeline snippet:**

```yaml
embed:
  adapter: ollama
  model: nomic-embed-text
  base_url: "${OLLAMA_URL}"
generation:
  adapter: ollama
  model: llama3
  base_url: "${OLLAMA_URL}"
```

**Streaming query:**

```bash
curl -N -X POST http://localhost:8081/v1/query/stream \
  -H 'Content-Type: application/json' \
  -d '{"query":"How do I run the pipeline?"}'
# SSE events: sources → token (×N) → done
```

---

Each new adapter auto-appears in GUI via registry (`rustyrag adapters list`):

- ✅ Semantic chunking (`chunk.adapter: semantic`)
- ✅ Hybrid search (dense + BM25 rerank)
- ✅ PDF + HTML parsing (via `parse.adapter: auto`)
- ✅ Ollama (local embeddings + LLM)
- ✅ ETL checkpoint/resume (per-document state save)
- ✅ Streaming `/v1/query/stream` responses (SSE)
- ✅ Safety: file size limits, max chunks, embed cost estimates in dry-run
- ⏳ S3 **source**, Confluence API, dedicated reranker adapter, SQLite checkpoint store

---

### Phase 7 — Vector store adapters ⏳

**Goal:** Extend `store.adapter` / `retrieval.store_adapter` beyond Qdrant so pipelines and query API can target Postgres, AWS OpenSearch, or S3 Vectors via config alone.

| Adapter | Work |
|---|---|
| **pgvector** | `sqlx` + `pgvector` crate; table DDL migration helper; HNSW/IVFFlat index options |
| **opensearch** | `opensearch` Rust client; k-NN index mapping; AWS SigV4 auth |
| **s3_vectors** | AWS SDK for S3 Vectors API; put/query vectors; IAM role support |

**Exit criteria:**

- Same ETL + query flow works end-to-end with each backend (at least one sample config per adapter).
- `rustyrag adapters list` and validator accept all four store adapters.
- RAG config `retrieval.store_adapter` selects query-time backend (replacing Qdrant-only `store_url` pattern).

**Not in scope:** dual-write to multiple stores in one pipeline run (use separate pipeline configs instead).

---

## Key decisions (locked)

| Decision | Choice |
|---|---|
| Project path | `~/coding/rustyrag` |
| Runtime config | YAML (GUI generates it) |
| TOML | Deferred |
| One pipeline | One logical index on **one** store target (collection/table/index name) |
| Vector store v1 | Qdrant implemented; pgVector, OpenSearch, S3 Vectors via same `VectorStoreAdapter` trait |
| Runtime split | Batch ETL worker + separate query API |
| Deployment | Container platform, horizontal scale |
| Secrets v1 | `.env` / platform env vars |

## Out of scope (for now)

- Multi-tenancy
- Vault / secret managers
- Docker image of RustyRAG (until Phase 5)

---

## Progress checklist

- [x] Phase 0 — Scaffold
- [x] Phase 1 — ETL vertical slice
- [x] Phase 2 — Query API
- [ ] Phase 3 — Config schema + adapter registry for GUI
- [ ] Phase 4 — Web GUI
- [ ] Phase 5 — Docker + multi-instance
- [x] Phase 6 — Advanced adapters (partial: semantic, hybrid, Ollama, PDF/HTML, streaming, safety)
- [ ] Phase 7 — Vector store adapters (pgVector, OpenSearch, S3 Vectors)

---

## Starting a new Cursor chat

1. Open folder `~/coding/rustyrag`
2. Say: *“Continue RustyRAG — see PLAN.md. Phase 6 adapters done; next is Phase 3 (GUI foundation) or Phase 5 (Docker).”*

The repo + this file are the source of truth — not chat history.

---

## Cursor plan file (internal)

Cursor also keeps a copy at:

`~/.cursor/plans/rust_rag_etl_framework_ee18665c.plan.md`

Use **`PLAN.md` in the repo** for day-to-day reading; it should stay in sync with that file.
