#!/bin/bash

# TimberDB Test Runner
# Runs all tests in sequence

echo "🌲 TimberDB Test Suite"
echo "====================="
echo

echo "🚀 Test 1: Writing test data"
echo "=========================="
./test_write.sh

echo
echo "🚀 Test 2: Reading test data"
echo "=========================="
./test_read.sh

# Only run API tests if API feature is available
if [ -f "../target/release/timberdb" ] && ../target/release/timberdb --help | grep -q "serve"; then
    echo
    echo "🚀 Test 3: API tests"
    echo "=================="
    ./test_api.sh
else
    echo
    echo "ℹ️  Skipping API tests (API feature not available)"
    echo "   Build with: cargo build --release --features api"
fi

echo
echo "🎉 All tests completed!"
