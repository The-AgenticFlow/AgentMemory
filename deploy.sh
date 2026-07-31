#!/bin/bash
set -e

# Arguments from workflow
SSH_USER="$1"
SSH_HOST="$2"
IMAGE="$3"
DEPLOY_DIR="/home/$SSH_USER/engram"

echo "=== Deployment Configuration ==="
echo "Image: $IMAGE"
echo "Deploy directory: $DEPLOY_DIR"
echo "SSH Host: $SSH_HOST"
echo ""

# NOTE: this script runs ON the target server already (invoked via ssh
# from the GitHub Actions workflow), so all commands below run locally.

mkdir -p "$DEPLOY_DIR"
cd "$DEPLOY_DIR"

echo "Stopping existing engram stack (scoped to this compose project only)..."
docker compose down --remove-orphans 2>/dev/null || true
sleep 3

echo "Backing up data volume..."
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
VOLUME_NAME="$(basename "$DEPLOY_DIR")_engram-data"
docker run --rm -v "${VOLUME_NAME}:/data:ro" -v "/tmp:/backup" alpine \
  tar -czf "/backup/engram_data_backup_$TIMESTAMP.tar.gz" -C /data . 2>/dev/null \
  && echo "Backup saved to /tmp/engram_data_backup_$TIMESTAMP.tar.gz" \
  || echo "No existing data volume to back up (first deploy)"

echo "Pulling and tagging Docker image..."
docker pull "$IMAGE" && docker tag "$IMAGE" engram-agent:latest

echo "Starting containers..."
docker compose up -d --remove-orphans --force-recreate

echo "Waiting 60 seconds for services to start..."
sleep 60

echo "Verifying services..."
docker compose exec -T neo4j wget -qO- --tries=1 http://localhost:7474 >/dev/null 2>&1 \
  && echo "Neo4j OK" || echo "Neo4j not ready yet"
curl -f http://localhost:3001/health || echo "Engram not ready yet"
curl -f http://localhost:3000 || echo "Grafana not ready yet"
curl -sfk https://localhost -o /dev/null && echo "Caddy OK" || echo "Caddy not ready yet (check caddy/certs/origin*.pem exist on the server)"
echo "Recent engram logs (for quick debugging):"
docker compose logs --tail=40 engram || true
echo "Recent caddy logs (for quick debugging):"
docker compose logs --tail=20 caddy || true

docker image prune -f 2>/dev/null || true
echo "=== Deployment complete ==="
