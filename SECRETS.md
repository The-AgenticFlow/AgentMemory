# Engram Deployment Secrets Guide

Configure these secrets in GitHub repository **Settings > Secrets and variables > Actions**

Repository secrets are encrypted and safe to use in CI/CD pipelines.

## Required Secrets for Deployment

### 1. Deploy Target (SSH)
```
DEPLOY_HOST=your-server.com         # Server hostname or IP
DEPLOY_USER=deploy                  # SSH username
DEPLOY_SSH_KEY=<private-key>        # SSH private key (for deploy user)
DEPLOY_PATH=/opt/engram             # Deployment directory on server (optional)
```

## LLM Configuration (via GitHub Secrets)

**Choose ONE LLM provider:**

### Option 1: OpenAI-Compatible Endpoint
```
LLM_ENDPOINT=https://api.openai.com/v1
LLM_API_KEY=sk-...
LLM_MODEL=gpt-4
```

### Option 2: Ollama (Local)
```
LLM_ENDPOINT=http://localhost:11434/v1
LLM_API_KEY=                        # Leave empty
LLM_MODEL=llama3.2
```

### Option 3: DashScope (Legacy)
```
ENGRAM_DASHSCOPE_API_KEY=your-api-key
LLM_MODEL=qwen-plus
```

### Optional LLM Settings
```
LLM_CHAT_PATH=chat/completions                          # Default
LLM_EMBEDDINGS_PATH=embeddings                          # Default
LLM_RERANK_ENDPOINT=https://api.example.com/rerank      # Optional
ENGRAM_REQUIRE_LLM=false                                # Optional
```

## Caddy (HTTPS reverse proxy)

Caddy runs as a container (`caddy` service in `docker-compose.yml`) and terminates TLS in
front of `engram`, using the `Caddyfile` at the repo root.

```
ENGRAM_DOMAIN=engram.example.com   # GitHub secret, optional (defaults to "localhost")
```

**Certificates are NOT managed via GitHub Secrets or git.** `caddy/certs/` is gitignored and
excluded from the deploy file transfer, so whatever you place there on the server persists
across deploys. One-time setup on the server:

```bash
mkdir -p ~/engram/caddy/certs
# Copy your real origin cert + key here (e.g. a Cloudflare Origin CA certificate pair):
#   ~/engram/caddy/certs/origin.pem
#   ~/engram/caddy/certs/origin-key.pem
```

If these files are missing, the `caddy` container will fail to start (the other services are
unaffected) — check with `docker compose logs caddy` on the server.

## Neo4j Configuration (Local .env only)

⚠️ **Neo4j configuration is NOT stored in GitHub Secrets**

Instead, configure Neo4j in the `.env` file on your deployment server:

```bash
# On your server at: /opt/engram/.env
ENGRAM_NEO4J_URI=http://neo4j:7474
ENGRAM_NEO4J_USER=neo4j
ENGRAM_NEO4J_PASSWORD=your-neo4j-password
ENGRAM_NEO4J_DATABASE=neo4j

# Neo4j service is typically configured via docker-compose.yml
# and persists in volumes, so credentials only need to be set once
```

## Example: Minimal Production Setup

```bash
# GitHub Actions Secrets:
DEPLOY_HOST=prod.example.com
DEPLOY_USER=deploy
DEPLOY_SSH_KEY=-----BEGIN OPENSSH PRIVATE KEY-----
...
-----END OPENSSH PRIVATE KEY-----
DEPLOY_PATH=/opt/engram
LLM_ENDPOINT=http://localhost:11434/v1
LLM_API_KEY=
LLM_MODEL=llama3.2

# Local server .env file (created once, persists):
ENGRAM_NEO4J_URI=http://neo4j:7474
ENGRAM_NEO4J_USER=neo4j
ENGRAM_NEO4J_PASSWORD=secure-password
```

## Deployment Flow

1. **GitHub Actions** passes LLM config from secrets to the server
2. **Deploy script** updates only the LLM configuration in `.env`
3. **Local .env** on server maintains Neo4j config unchanged
4. **docker-compose** reads the complete `.env` file with both LLM + Neo4j config

## Deployment Triggers

The pipeline runs automatically when:
- Push to `main`, `master`, or `release/*` branches
- Merge of a pull request

The pipeline is skipped for:
- Feature branches (unless pushed to release/*)
- Draft pull requests