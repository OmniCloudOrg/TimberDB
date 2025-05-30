#!/bin/bash

# TimberDB Test Scripts Setup
# This script sets up the test environment and makes scripts executable

echo "🌲 TimberDB Test Scripts Setup"
echo "=============================="
echo

# Make scripts executable
echo "📝 Making test scripts executable..."
chmod +x test_write.sh
chmod +x test_read.sh
chmod +x test_api.sh

echo "✅ Scripts are now executable"
echo

# Check if TimberDB is built
echo "🔍 Checking TimberDB build..."
if [ ! -f "./target/release/timberdb" ]; then
    echo "⚠️  TimberDB not found in release mode"
    echo "   Building TimberDB..."
    
    # Build without API first
    echo "   Building CLI version..."
    cargo build --release
    
    # Check if API feature should be built
    if grep -q 'features.*api' Cargo.toml 2>/dev/null; then
        echo "   Building with API feature..."
        cargo build --release --features api
    fi
    
    echo "✅ Build completed"
else
    echo "✅ TimberDB binary found"
fi

echo

# Check dependencies for API tests
echo "🔍 Checking dependencies..."

# Check for curl
if command -v curl &> /dev/null; then
    echo "✅ curl found"
else
    echo "❌ curl not found - needed for API tests"
    echo "   Install: sudo apt install curl (Ubuntu/Debian) or brew install curl (macOS)"
fi

# Check for jq (optional but recommended)
if command -v jq &> /dev/null; then
    echo "✅ jq found"
else
    echo "⚠️  jq not found - JSON responses won't be formatted nicely"
    echo "   Install: sudo apt install jq (Ubuntu/Debian) or brew install jq (macOS)"
fi

# Check for uuidgen
if command -v uuidgen &> /dev/null; then
    echo "✅ uuidgen found"
else
    echo "⚠️  uuidgen not found - using alternative ID generation"
fi

echo

# Create a simple test runner script
echo "📋 Creating test runner script..."
cat > run_tests.sh << 'EOF'
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
if [ -f "./target/release/timberdb" ] && ./target/release/timberdb --help | grep -q "serve"; then
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
EOF

chmod +x run_tests.sh

echo "✅ Test runner created: run_tests.sh"
echo

# Summary
echo "📋 Available Test Scripts:"
echo "========================="
echo "   test_write.sh  - Writes test data to TimberDB"
echo "   test_read.sh   - Queries and reads test data"
echo "   test_api.sh    - Tests HTTP API endpoints"
echo "   run_tests.sh   - Runs all tests in sequence"
echo

echo "🚀 Quick Start:"
echo "==============="
echo "   1. Run all tests:           ./run_tests.sh"
echo "   2. Or run individually:"
echo "      - Write test data:       ./test_write.sh"
echo "      - Read/query data:       ./test_read.sh"
echo "      - Test API:              ./test_api.sh"
echo

echo "💡 Tips:"
echo "========"
echo "   - Test data is stored in ./test_data/"
echo "   - Delete ./test_data/ to start fresh"
echo "   - API tests require the 'api' feature"
echo "   - Check logs with: RUST_LOG=debug ./target/release/timberdb ..."
echo

echo "✅ Setup complete! Ready to run tests."