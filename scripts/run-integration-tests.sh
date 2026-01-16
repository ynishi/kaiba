#!/bin/bash
# Run integration tests with Docker services
#
# Usage: ./scripts/run-integration-tests.sh [--keep]
#   --keep: Keep containers running after tests

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
COMPOSE_FILE="$PROJECT_ROOT/docker-compose.test.yml"

# Parse arguments
KEEP_CONTAINERS=false
for arg in "$@"; do
    case $arg in
        --keep)
            KEEP_CONTAINERS=true
            shift
            ;;
    esac
done

echo "🚀 Starting test containers..."
docker compose -f "$COMPOSE_FILE" up -d

echo "⏳ Waiting for services to be healthy..."

# Wait for Neo4j
echo "  Waiting for Neo4j..."
for i in {1..30}; do
    if docker compose -f "$COMPOSE_FILE" exec -T neo4j wget -q -O - http://localhost:7474 > /dev/null 2>&1; then
        echo "  ✅ Neo4j is ready"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "  ❌ Neo4j failed to start"
        docker compose -f "$COMPOSE_FILE" logs neo4j
        exit 1
    fi
    sleep 2
done

# Wait for Qdrant
echo "  Waiting for Qdrant..."
for i in {1..20}; do
    if curl -s http://localhost:6333/healthz > /dev/null 2>&1; then
        echo "  ✅ Qdrant is ready"
        break
    fi
    if [ $i -eq 20 ]; then
        echo "  ❌ Qdrant failed to start"
        docker compose -f "$COMPOSE_FILE" logs qdrant
        exit 1
    fi
    sleep 1
done

echo ""
echo "🧪 Running integration tests..."

# Set environment variables for tests
export NEO4J_TEST_URI="bolt://localhost:7687"
export NEO4J_TEST_USER="neo4j"
export NEO4J_TEST_PASSWORD="testpassword"
export QDRANT_TEST_URL="http://localhost:6333"

# Run integration tests with feature flag
cd "$PROJECT_ROOT"
cargo test -p kaiba-server --features integration -- --nocapture

TEST_EXIT_CODE=$?

if [ "$KEEP_CONTAINERS" = false ]; then
    echo ""
    echo "🧹 Cleaning up containers..."
    docker compose -f "$COMPOSE_FILE" down -v
fi

if [ $TEST_EXIT_CODE -eq 0 ]; then
    echo ""
    echo "✅ All integration tests passed!"
else
    echo ""
    echo "❌ Some tests failed"
fi

exit $TEST_EXIT_CODE
