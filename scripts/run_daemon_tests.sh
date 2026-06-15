#!/bin/bash
# Run pmix-rs tests that require a PMIx daemon.
# Reads the PRTE URI from the systemd-managed service and exports PMIX_SERVER_URI.
#
# Usage:
#   ./scripts/run_daemon_tests.sh          # Run all daemon tests
#   ./scripts/run_daemon_tests.sh TOOL     # Run only tool_tool_init tests
#   ./scripts/run_daemon_tests.sh LIB      # Run only lib_core_api daemon tests
#   ./scripts/run_daemon_tests.sh FABRIC   # Run only fabric daemon tests
#   ./scripts/run_daemon_tests.sh COV      # Run coverage with daemon tests included
#
# Prerequisites:
#   systemctl --user start prte

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
URI_FILE="/run/user/1000/prte/uri"

# Check PRTE is running
if ! systemctl --user is-active prte &>/dev/null; then
    echo "ERROR: prte systemd service is not running."
    echo "Start it with: systemctl --user start prte"
    exit 1
fi

# Read URI
if [ ! -f "$URI_FILE" ]; then
    echo "ERROR: PRTE URI file not found at $URI_FILE"
    echo "The prte service may not have started properly."
    exit 1
fi

PMIX_SERVER_URI=$(head -1 "$URI_FILE")
echo "PRTE URI: $PMIX_SERVER_URI"

# Verify connectivity with a quick C test
export PMIX_SERVER_URI
export PMIX_TEST_URI_FILE="$URI_FILE"

cd "$PROJECT_DIR"

case "${1:-ALL}" in
    TOOL)
        echo "Running tool_tool_init daemon tests..."
        cargo test --test tool_tool_init -- --ignored
        ;;
    LIB)
        echo "Running lib_core_api daemon tests..."
        cargo test --test lib_core_api -- --ignored
        ;;
    FABRIC)
        echo "Running fabric_fabric_comprehensive daemon tests..."
        cargo test --test fabric_fabric_comprehensive -- --ignored
        ;;
    COV)
        echo "Running coverage with daemon tests..."
        cargo llvm-cov --json -- --ignored
        ;;
    ALL)
        echo "Running all daemon-dependent tests..."
        cargo test --test tool_tool_init -- --ignored
        cargo test --test lib_core_api -- --ignored
        cargo test --test fabric_fabric_comprehensive -- --ignored
        ;;
    *)
        echo "Usage: $0 [TOOL|LIB|FABRIC|COV|ALL]"
        exit 1
        ;;
esac
