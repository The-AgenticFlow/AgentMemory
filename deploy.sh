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

echo "Stopping existing containers..."
docker stop $(docker ps -aq) 2>/dev/null || true
docker rm $(docker ps -aq) 2>/dev/null || true
docker compose down --remove-orphans 2>/dev/null || true
fuser -k 7474/tcp 7687/tcp 2>/dev/null || lsof -t -i :7474 -i :7687 | xargs -r kill -9 2>/dev/null || true
sleep 5

echo "Backing up data..."
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
cp -r "$DEPLOY_DIR/data" "/tmp/engram_data_backup_$TIMESTAMP" 2>/dev/null || true

echo "Pulling and tagging Docker image..."
docker pull "$IMAGE" && docker tag "$IMAGE" engram-agent:latest

echo "Starting containers..."
docker compose up -d --remove-orphans --force-recreate

echo "Waiting 60 seconds for services to start..."
sleep 60

echo "Verifying services..."
curl -sf http://localhost:7474 || echo "Neo4j not ready yet"
curl -f http://localhost:3001/health || echo "Engram not ready yet"
curl -f http://localhost:3000 || echo "Grafana not ready yet"

docker image prune -f 2>/dev/null || true
echo "=== Deployment complete ==="
