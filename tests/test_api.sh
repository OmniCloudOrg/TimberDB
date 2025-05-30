#!/bin/bash

# TimberDB API Test Script
# This script tests the HTTP API endpoints

set -e

echo "🌲 TimberDB API Test Script"
echo "=========================="
echo

# Configuration
API_BASE="http://127.0.0.1:8080"
DATA_DIR="./test_data"
TIMBERDB_BIN="../target/release/timberdb"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to make API calls and format output
api_call() {
    local method="$1"
    local endpoint="$2"
    local data="$3"
    local description="$4"
    
    echo -e "${BLUE}🌐 $description${NC}"
    echo "   $method $endpoint"
    
    if [ -n "$data" ]; then
        echo "   Data: $data"
    fi
    
    echo "   Response:"
    echo "   ---------"
    
    local cmd="curl -s -X $method"
    if [ -n "$data" ]; then
        cmd="$cmd -H 'Content-Type: application/json' -d '$data'"
    fi
    cmd="$cmd $API_BASE$endpoint"
    
    # Execute the curl command and format JSON output
    if response=$(eval $cmd); then
        echo "$response" | jq . 2>/dev/null || echo "$response"
        echo -e "   ${GREEN}✅ Success${NC}"
    else
        echo -e "   ${RED}❌ Failed${NC}"
    fi
    
    echo
}

# Check if TimberDB binary exists
if [ ! -f "$TIMBERDB_BIN" ]; then
    echo -e "${RED}❌ TimberDB binary not found at $TIMBERDB_BIN${NC}"
    echo "   Please build the project first: cargo build --release --features api"
    exit 1
fi

# Check if jq is installed for JSON formatting
if ! command -v jq &> /dev/null; then
    echo -e "${YELLOW}⚠️  jq not found. JSON responses will not be formatted.${NC}"
    echo "   Install jq for better output: sudo apt install jq"
    echo
fi

# Start the API server in background
echo -e "${BLUE}🚀 Starting TimberDB API server...${NC}"
$TIMBERDB_BIN --data-dir "$DATA_DIR" serve --bind "127.0.0.1:8080" &
SERVER_PID=$!

# Function to cleanup on exit
cleanup() {
    echo
    echo -e "${YELLOW}🛑 Stopping API server...${NC}"
    kill $SERVER_PID 2>/dev/null || true
    wait $SERVER_PID 2>/dev/null || true
}
trap cleanup EXIT

# Wait for server to start
echo "   Waiting for server to start..."
sleep 3

# Check if server is responding
if ! curl -s "$API_BASE/health" > /dev/null; then
    echo -e "${RED}❌ API server is not responding${NC}"
    echo "   Make sure the server started correctly"
    exit 1
fi

echo -e "${GREEN}✅ API server is running${NC}"
echo

# Test API endpoints
echo -e "${BLUE}🧪 Running API Tests:${NC}"
echo "=================="
echo

# Test 1: Health check
api_call "GET" "/health" "" "Health check"

# Test 2: List partitions
api_call "GET" "/api/v1/partitions" "" "List all partitions"

# Test 3: Create a new partition
api_call "POST" "/api/v1/partitions" '{"name": "api-test-logs"}' "Create new partition"

# Get the partition ID for further tests
echo -e "${BLUE}📋 Getting partition ID for further tests...${NC}"
PARTITION_RESPONSE=$(curl -s "$API_BASE/api/v1/partitions")
PARTITION_ID=$(echo "$PARTITION_RESPONSE" | jq -r '.[] | select(.name == "api-test-logs") | .id' 2>/dev/null || echo "")

if [ -z "$PARTITION_ID" ]; then
    echo -e "${RED}❌ Could not get partition ID${NC}"
    exit 1
fi

echo "   Using partition ID: $PARTITION_ID"
echo

# Test 4: Get specific partition info
api_call "GET" "/api/v1/partitions/$PARTITION_ID" "" "Get partition info"

# Test 5: Append log entries
echo -e "${BLUE}📝 Appending test log entries...${NC}"

api_call "POST" "/api/v1/logs/$PARTITION_ID" '{
    "source": "api-test-service",
    "message": "Test log entry from API",
    "tags": {
        "level": "info",
        "test": "true",
        "api_version": "v1"
    }
}' "Append info log entry"

api_call "POST" "/api/v1/logs/$PARTITION_ID" '{
    "source": "api-test-service",
    "message": "Error occurred during API test",
    "tags": {
        "level": "error",
        "test": "true",
        "error_code": "TEST_ERROR"
    }
}' "Append error log entry"

api_call "POST" "/api/v1/logs/$PARTITION_ID" '{
    "source": "api-gateway",
    "message": "API request processed successfully",
    "tags": {
        "level": "info",
        "method": "POST",
        "path": "/api/v1/test",
        "status": "200"
    }
}' "Append API gateway log"

# Test 6: Get specific log entry
api_call "GET" "/api/v1/logs/$PARTITION_ID/0" "" "Get first log entry"

# Test 7: Query logs
api_call "POST" "/api/v1/query" '{
    "partitions": ["'$PARTITION_ID'"],
    "limit": 10
}' "Query logs from test partition"

# Test 8: Query logs with filters
api_call "POST" "/api/v1/query" '{
    "partitions": ["'$PARTITION_ID'"],
    "tags": {
        "level": "error"
    },
    "limit": 5
}' "Query error logs only"

# Test 9: Query logs by source
api_call "POST" "/api/v1/query" '{
    "partitions": ["'$PARTITION_ID'"],
    "source": "api-test-service",
    "limit": 5
}' "Query logs by source"

# Test 10: Query logs with message filter
api_call "POST" "/api/v1/query" '{
    "message_contains": "API",
    "limit": 10
}' "Query logs containing 'API'"

# Test 11: Query with time range (last hour)
CURRENT_TIME=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
ONE_HOUR_AGO=$(date -u -d "1 hour ago" +"%Y-%m-%dT%H:%M:%SZ")

api_call "POST" "/api/v1/query" '{
    "time_range": {
        "start": "'$ONE_HOUR_AGO'",
        "end": "'$CURRENT_TIME'"
    },
    "limit": 20
}' "Query logs from last hour"

# Test 12: Get metrics
api_call "GET" "/api/v1/metrics" "" "Get system metrics"

# Test error cases
echo -e "${BLUE}🚨 Testing Error Cases:${NC}"
echo "======================"
echo

# Test 13: Invalid partition ID
api_call "GET" "/api/v1/partitions/invalid-id" "" "Get non-existent partition (should fail)"

# Test 14: Invalid log entry ID
api_call "GET" "/api/v1/logs/$PARTITION_ID/999999" "" "Get non-existent log entry (should fail)"

# Test 15: Invalid request body
api_call "POST" "/api/v1/partitions" '{"invalid": "json"}' "Create partition with invalid data (should fail)"

# Test 16: Empty partition name
api_call "POST" "/api/v1/partitions" '{"name": ""}' "Create partition with empty name (should fail)"

echo -e "${BLUE}📊 Performance Test:${NC}"
echo "===================="
echo

# Performance test - multiple concurrent requests
echo -e "${BLUE}⚡ Testing concurrent requests...${NC}"
echo "   Making 10 concurrent requests..."

for i in {1..10}; do
    curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "{\"source\": \"perf-test-$i\", \"message\": \"Performance test entry $i\", \"tags\": {\"test\": \"performance\", \"batch\": \"$i\"}}" \
        "$API_BASE/api/v1/logs/$PARTITION_ID" > /dev/null 2>&1 &
done

# Simple wait with timeout
echo "   Waiting for requests to complete..."
sleep 3  # Give requests time to complete

# Kill any remaining curl processes
pkill -f "curl.*$API_BASE" 2>/dev/null || true

echo -e "${GREEN}✅ Concurrent requests completed${NC}"✅ Concurrent requests completed${NC}"
echo

# Final stats
api_call "GET" "/api/v1/partitions" "" "Final partition list"

echo -e "${GREEN}🎉 API tests completed!${NC}"
echo
echo -e "${BLUE}💡 API Documentation:${NC}"
echo "   Health Check:     GET  /health"
echo "   List Partitions:  GET  /api/v1/partitions"
echo "   Create Partition: POST /api/v1/partitions"
echo "   Get Partition:    GET  /api/v1/partitions/{id}"
echo "   Append Log:       POST /api/v1/logs/{partition_id}"
echo "   Get Log Entry:    GET  /api/v1/logs/{partition_id}/{entry_id}"
echo "   Query Logs:       POST /api/v1/query"
echo "   Get Metrics:      GET  /api/v1/metrics"
echo
echo -e "${BLUE}🔧 Test your own API calls:${NC}"
echo "   curl -X GET $API_BASE/health"
echo "   curl -X GET $API_BASE/api/v1/partitions"