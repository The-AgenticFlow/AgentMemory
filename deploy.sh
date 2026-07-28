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

mkdir -p "$DEPLOY_DIR"

echo "Stopping existing containers..."
ssh "$SSH_USER@$SSH_HOST" "docker stop \$(docker ps -aq) 2>/dev/null || true"
ssh "$SSH_USER@$SSH_HOST" "docker rm \$(docker ps -aq) 2>/dev/null || true"
ssh "$SSH_USER@$SSH_HOST" "cd '$DEPLOY_DIR' && docker compose down --remove-orphans 2>/dev/null || true"
ssh "$SSH_USER@$SSH_HOST" "fuser -k 7474/tcp 7687/tcp 2>/dev/null || lsof -t -i :7474 -i :7687 | xargs -r kill -9 2>/dev/null || true"
ssh "$SSH_USER@$SSH_HOST" "sleep 5"

echo "Backing up data..."
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
ssh "$SSH_USER@$SSH_HOST" "cp -r '$DEPLOY_DIR/data' /tmp/engram_data_backup_\$TIMESTAMP 2>/dev/null || true"

echo "Transferring files..."
tar --exclude=.git --exclude=.agents --exclude=target --exclude=web/node_modules --exclude=web/dist -czf - . | ssh "$SSH_USER@$SSH_HOST" "cd '$DEPLOY_DIR' && tar -xzf -"

echo "Pulling and tagging Docker image..."
ssh "$SSH_USER@$SSH_HOST" "docker pull $IMAGE && docker tag $IMAGE engram-agent:latest"

echo "Starting containers..."
ssh "$SSH_USER@$SSH_HOST" "cd '$DEPLOY_DIR' && docker compose up -d --remove-orphans --force-recreate"

echo "Waiting 60 seconds for services to start..."
sleep 60

echo "Verifying services..."
ssh "$SSH_USER@$SSH_HOST" "curl -sf http://localhost:7474 || echo Neo4j not ready yet"
ssh "$SSH_USER@$SSH_HOST" "curl -f http://localhost:3001/health || echo Engram not ready yet"
ssh "$SSH_USER@$SSH_HOST" "curl -f http://localhost:3000 || echo Grafana not ready yet"

ssh "$SSH_USER@$SSH_HOST" "docker image prune -f 2>/dev/null || true"
echo "=== Deployment complete ==="