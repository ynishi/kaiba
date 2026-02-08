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

Cloud Build checks `.gcloudignore` first, then falls back to `.gitignore` when uploading source.
If `.gitignore` contains `Cargo.lock`, the build will fail.

Place a `.gcloudignore` at the repository root and **do not** exclude `Cargo.lock`.

```gitignore
# .gcloudignore
target/
.git/
.github/
.claude/
.shuttle/
docs/
examples/
*.md
.env
.env.*
Secrets.toml
**/Secrets.toml
.DS_Store
.vscode/
.idea/
node_modules/
```

## Dockerfile

No `--platform` flag is needed in `FROM` — Cloud Build runs on amd64 natively.
Adding `--platform linux/amd64` for local builds triggers QEMU emulation and is extremely slow.
Use Cloud Build even for local deployments when possible.

## Environment Variables

Inject all environment variables via Secret Manager.
Mixing plain env vars and secrets can cause type-conflict errors on Cloud Run.

| Variable | Secret Manager Key | Required | Purpose |
|:---------|:-------------------|:---------|:--------|
| `DATABASE_URL` | `database-url` | Yes | PostgreSQL (Cloud SQL Proxy) |
| `KAIBA_API_KEY` | `kaiba-api-key` | No | API authentication |
| `QDRANT_URL` | `qdrant-url` | No | Qdrant endpoint |
| `QDRANT_API_KEY` | `qdrant-api-key` | No | Qdrant authentication |
| `OPENAI_API_KEY` | `openai-api-key` | No | OpenAI Embedding |
| `GEMINI_API_KEY` | `gemini-api-key` | No | Gemini WebSearch |
| `NEO4J_URI` | `neo4j-uri` | No | Neo4j endpoint |
| `NEO4J_USER` | `neo4j-user` | No | Neo4j user |
| `NEO4J_PASSWORD` | `neo4j-password` | No | Neo4j authentication |
| `GROQ_API_KEY` | `groq-api-key` | No | Groq Decision Engine |

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
