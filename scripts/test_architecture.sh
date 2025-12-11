#!/bin/bash
# scripts/test_architecture.sh

echo "========================================"
echo "    Inventiv-Agents Infra Tester"
echo "========================================"

BASE_ORCHESTRATOR="http://localhost:8001"
BASE_ROUTER="http://localhost:8002"
BASE_BACKEND="http://localhost:8003"

check_service() {
    NAME=$1
    URL=$2
    echo -n "Checking $NAME ($URL)... "
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "$URL")
    if [ "$CODE" == "200" ]; then
        echo "✅ OK"
    else
        echo "❌ FAIL (Code: $CODE)"
    fi
}

# 1. Health Checks
echo "[1] Service Availability"
check_service "Orchestrator" "$BASE_ORCHESTRATOR/"
check_service "Router" "$BASE_ROUTER/health"
check_service "Backend" "$BASE_BACKEND/health"

# 2. Functional Tests
echo -e "\n[2] Functional Tests"

# 2.1 Orchestrator Status
echo -n "Fetching Cluster Status... "
STATUS=$(curl -s "$BASE_ORCHESTRATOR/admin/status")
echo "$STATUS" | grep "cloud_instances_count" > /dev/null
if [ $? -eq 0 ]; then
    echo "✅ OK"
    echo "   Response: $STATUS"
else
    echo "❌ FAIL"
    echo "   Response: $STATUS"
fi

# 2.2 Router Proxy (Expect 502 or 200 depending on worker existence)
echo -n "Testing Inference Proxy (Router -> ???)... "
# Note: This is expected to fail (502) if no worker is running on localhost:8000
# But we want to ensure the Router *tries* and responds.
RESPONSE=$(curl -s -w "\n%{http_code}" -X POST "$BASE_ROUTER/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -d '{"model": "test-model", "messages": []}')

HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
BODY=$(echo "$RESPONSE" | head -n-1)

if [ "$HTTP_CODE" == "502" ]; then
    echo "✅ OK (Got 502 as expected for missing worker)"
elif [ "$HTTP_CODE" == "200" ]; then
    echo "✅ OK (Worker responded!)"
else
    echo "⚠️  Unexpected Code: $HTTP_CODE"
    echo "   Response: $BODY"
fi

echo -e "\n========================================"
