# Deploying to Google Cloud Run

Reference guide for deploying Kaiba Server to Google Cloud Run.

## DATABASE_URL Format

When connecting from Cloud Run to Cloud SQL via Cloud SQL Proxy (Unix socket),
sqlx (PgPool) fails to parse the URL if the host portion is empty.

```
# WRONG: sqlx raises "empty host" error
postgresql://YOUR_USER:YOUR_PASSWORD@/kaiba?host=/cloudsql/YOUR_PROJECT:YOUR_REGION:YOUR_INSTANCE

# OK: use localhost as a dummy host; the host parameter overrides it with the Unix socket
postgresql://YOUR_USER:YOUR_PASSWORD@localhost/kaiba?host=/cloudsql/YOUR_PROJECT:YOUR_REGION:YOUR_INSTANCE
```

If you store `database-url` in Secret Manager, make sure it uses the `@localhost/` form.

## .gcloudignore

Ref repository root.


## Dockerfile

No `--platform` flag is needed in `FROM` — Cloud Build runs on amd64 natively.
Adding `--platform linux/amd64` for local builds triggers QEMU emulation and is extremely slow.
Use Cloud Build even for local deployments when possible.

## Environment Variables

Inject all environment variables via Secret Manager.
Mixing plain env vars and secrets can cause type-conflict errors on Cloud Run.

| Variable | Required | Purpose |
|:---------|:---------|:--------|
| `DATABASE_URL` | Yes | PostgreSQL (Cloud SQL Proxy) |
| `KAIBA_API_KEY` | No | API authentication |
| `QDRANT_URL` | No | Qdrant endpoint |
| `QDRANT_API_KEY` | No | Qdrant authentication |
| `OPENAI_API_KEY` | No | OpenAI Embedding |
| `GEMINI_API_KEY` | No | Gemini WebSearch |
| `NEO4J_URI` | No | Neo4j endpoint |
| `NEO4J_USER` | No | Neo4j user |
| `NEO4J_PASSWORD` | No | Neo4j authentication |
| `GROQ_API_KEY` | No | Groq Decision Engine |
| `OLLAMA_URL` | No | Ollama endpoint (local LLM fallback) |
| `OLLAMA_MODEL` | No | Ollama model name |
| `DECISION_PERSONA` | No | Persona context for Decision Engine |
| `LEARNING_INTERVAL_SECS` | No | Auto-learning scheduler interval (seconds) |

`PORT` is set automatically by Cloud Run (default 8080) — no Secret Manager entry needed.

## Recommended Cloud Run Specs

| Setting | Value |
|:--------|:------|
| CPU | 1 |
| Memory | 512Mi |
| Min instances | 0 (scale to zero) |
| Max instances | 3 |
| Timeout | 300s |
| Auth | unauthenticated (use API key) |

## Build

Using Cloud Build with `E2_HIGHCPU_8` machine type, builds take ~3 minutes.
The Dockerfile uses multi-stage builds to cache dependency crates.
