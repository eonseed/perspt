#!/bin/bash
set -e

echo "=========================================="
echo "Debug Long Response Streaming Issue"
echo "=========================================="

# Build the project first
echo "Building project with enhanced logging..."
cargo build

echo ""
echo "Setting up debug environment..."

# Create logs directory if it doesn't exist
mkdir -p logs

# Set environment variables for debug logging
export RUST_LOG=perspt=info
export RUST_BACKTRACE=1

# Get timestamp for log file
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
LOG_FILE="logs/debug_streaming_${TIMESTAMP}.log"

echo "Running with debug logging enabled..."
echo "Logs will be written to: $LOG_FILE"
echo ""
echo "To test the long response issue:"
echo "1. Ask for a very long response like 'Write a detailed 1000-word essay about artificial intelligence and its impact on society'"
echo "2. The application will run normally, but detailed logs will be captured"
echo "3. After testing, press Ctrl+C to stop and we'll analyze the logs"
echo ""
echo "Starting perspt with enhanced logging..."
echo ""

# Run the application and capture logs
./target/debug/perspt -p "google" -k "your_gemini_key"" -m gemini-2.0-flash 2>&1 | tee "$LOG_FILE"

echo ""
echo "=========================================="
echo "Log Analysis"
echo "=========================================="
echo "Log file saved to: $LOG_FILE"
echo ""
echo "Key patterns to look for:"
echo "- '=== STREAM START ===' and '=== STREAM COMPLETE ===' markers"
echo "- CHUNK processing logs showing progress"
echo "- Any '!!! STREAM ENDED IMPLICITLY !!!' warnings"
echo "- EOT signal sending and receiving"
echo ""
echo "Use this command to analyze the logs:"
echo "grep -E '(STREAM|CHUNK|EOT|ERROR)' '$LOG_FILE'"
