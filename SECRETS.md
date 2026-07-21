# Engram Deployment Secrets Guide
# ==================================
# Configure these secrets in GitHub repository Settings > Secrets and variables > Actions
# Repository secrets are encrypted and safe to use in CI/CD pipelines

## Required Secrets for Deployment

### Deploy Target (choose one: SSH or Kubernetes)

#### SSH Deployment
```
DEPLOY_HOST=your-server.com          # Server hostname or IP
DEPLOY_USER=deploy                  # SSH username
DEPLOY_SSH_KEY=<private-key>        # Private SSH key with deploy user access
DEPLOY_PATH=/opt/engram             # Deployment directory on server
```

#### Kubernetes Deployment
```
KUBECONFIG_DATA=<kubeconfig-base64> # Base64-encoded kubeconfig
K8S_NAMESPACE=engram                # Kubernetes namespace
```

## Optional: LLM Configuration

### OpenAI-Compatible Endpoint
```
LLM_ENDPOINT=https://api.openai.com/v1
LLM_API_KEY=sk-...
LLM_MODEL=gpt-4
```

### Or use Ollama (local)
```
LLM_ENDPOINT=http://localhost:11434/v1
LLM_API_KEY=
LLM_MODEL=llama3.2
```

### Or use DashScope (legacy)
```
ENGRAM_DASHSCOPE_API_KEY=your-api-key
LLM_MODEL=qwen-plus
```

### Custom Paths (optional)
```
LLM_CHAT_PATH=chat/completions
LLM_EMBEDDINGS_PATH=embeddings
LLM_RERANK_ENDPOINT=https://api.example.com/rerank
```

### Require LLM (optional)
```
ENGRAM_REQUIRE_LLM=false  # Set to true if chat requires LLM
```

## Required: Neo4j Configuration
```
ENGRAM_NEO4J_URI=http://neo4j:7474
ENGRAM_NEO4J_USER=neo4j
ENGRAM_NEO4J_PASSWORD=your-neo4j-password
ENGRAM_NEO4J_DATABASE=neo4j
```

## Example: Minimal Production Setup

```bash
# In GitHub Actions Secrets, add:
DEPLOY_HOST=prod.example.com
DEPLOY_USER=deploy
DEPLOY_SSH_KEY=<ssh-key>
DEPLOY_PATH=/opt/engram
LLM_ENDPOINT=http://localhost:11434/v1
LLM_API_KEY=
LLM_MODEL=llama3.2
ENGRAM_NEO4J_URI=http://neo4j:7474
ENGRAM_NEO4J_USER=neo4j
ENGRAM_NEO4J_PASSWORD=secure-password
```

## Deployment Triggers

The deployment pipeline runs automatically when:
1. Push to `main`, `master`, or `release/*` branches
2. Merge of a pull request (closed + merged)

The pipeline is skipped for:
- Push to feature branches
- Draft pull requests