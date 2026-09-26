#!/bin/bash
# Run pmix-rs tests that require a PMIx daemon / DVM.
#
# Usage:
#   ./scripts/run_daemon_tests.sh              # Run all daemon tests
#   ./scripts/run_daemon_tests.sh TOOL         # tool_tool_init tests
#   ./scripts/run_daemon_tests.sh LIB          # lib_core_api daemon tests
#   ./scripts/run_daemon_tests.sh FABRIC       # fabric daemon tests
#   ./scripts/run_daemon_tests.sh THREADING    # multi-thread + external-progress (#54)
#   ./scripts/run_daemon_tests.sh COV          # coverage with daemon tests included
#
# Prerequisites (TOOL/LIB/FABRIC/ALL — tool URI path):
#   systemctl --user start prte   # or a PRTE system-server writing $URI_FILE
#
# Prerequisites (THREADING — DVM / prterun path):
#   OpenPMIx ≥ 6.1 and PRTE ≥ 4.1 on PATH (same libpmix), e.g.:
#     export PMIX_PREFIX=$HOME/.local/openpmix-6.1.0
#     export PATH=/path/to/prte-4.1/bin:$PATH
#     export LD_LIBRARY_PATH=$PMIX_PREFIX/lib:$LD_LIBRARY_PATH
#   prterun must work: `prterun --version`

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
URI_FILE="${PMIX_TEST_URI_FILE:-/run/user/$(id -u)/prte/uri}"

require_uri_daemon() {
    if [ ! -f "$URI_FILE" ]; then
        # Best-effort: systemd unit may not exist on all hosts.
        if systemctl --user is-active prte &>/dev/null; then
            :
        elif systemctl --user list-unit-files 2>/dev/null | grep -q '^prte'; then
            echo "ERROR: prte systemd service is not running."
            echo "Start it with: systemctl --user start prte"
            exit 1
        else
            echo "ERROR: PRTE URI file not found at $URI_FILE"
            echo "Start a PRTE system-server that writes the URI, or set PMIX_TEST_URI_FILE."
            exit 1
        fi
    fi
    if [ ! -f "$URI_FILE" ]; then
        echo "ERROR: PRTE URI file not found at $URI_FILE"
        exit 1
    fi
    PMIX_SERVER_URI=$(head -1 "$URI_FILE")
    echo "PRTE URI: $PMIX_SERVER_URI"
    export PMIX_SERVER_URI
    export PMIX_TEST_URI_FILE="$URI_FILE"
}

require_prterun() {
    if ! command -v prterun >/dev/null 2>&1; then
        echo "ERROR: prterun not on PATH (need PRTE ≥ 4.1 built against OpenPMIx ≥ 6.1)."
        exit 1
    fi
    echo "Using prterun: $(command -v prterun)"
    prterun --version 2>&1 | head -3 || true
}

cd "$PROJECT_DIR"

case "${1:-ALL}" in
    TOOL)
        require_uri_daemon
        echo "Running tool_tool_init daemon tests..."
        cargo test --test tool_tool_init -- --ignored --test-threads=1
        ;;
    LIB)
        require_uri_daemon
        echo "Running lib_core_api daemon tests..."
        cargo test --test lib_core_api -- --ignored --test-threads=1
        ;;
    FABRIC)
        require_uri_daemon
        echo "Running fabric_fabric_comprehensive daemon tests..."
        cargo test --test fabric_fabric_comprehensive -- --ignored --test-threads=1
        ;;
    THREADING)
        require_prterun
        echo "Running multi-thread + external-progress tests (issue #54)..."
        # Build once, then run the test binary under prterun (avoids nested cargo
        # and lets us bound each case with timeout).
        # Resolve the exact executable via cargo JSON (deterministic; avoids
        # `ls <glob>` dying under set -e when the glob is empty).
        BIN=$(
            cargo test --test threading_mt_via_prterun --no-run --message-format=json \
                | python3 -c '
import json, sys
bin_path = None
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        msg = json.loads(line)
    except json.JSONDecodeError:
        continue
    if msg.get("reason") != "compiler-artifact":
        continue
    target = msg.get("target") or {}
    if "test" not in (target.get("kind") or []):
        continue
    name = target.get("name") or ""
    if name != "threading_mt_via_prterun" and not name.startswith("threading_mt_via_prterun"):
        continue
    exe = msg.get("executable")
    if exe:
        bin_path = exe
if not bin_path:
    sys.exit(1)
print(bin_path)
'
        ) || {
            echo "ERROR: could not locate threading_mt_via_prterun test binary"
            exit 1
        }
        if [ ! -x "$BIN" ]; then
            echo "ERROR: threading_mt_via_prterun test binary is not executable: $BIN"
            exit 1
        fi
        echo "Test binary: $BIN"
        # Standalone first (no DVM).
        "$BIN" --test-threads=1
        # cargo TESTNAME is a *substring* filter (not a regex). Each DVM case
        # runs under its own prterun so process-wide session state stays clean.
        for tname in \
            mt_concurrent_put_and_fence \
            mt_concurrent_fence_nb_completions \
            callback_must_not_block_progress_timeout \
            mt_external_progress_host_thread
        do
            echo "---- prterun: $tname ----"
            # 60s wall clock per case — external_progress hangs must fail closed.
            if command -v timeout >/dev/null 2>&1; then
                timeout 60 prterun -np 1 "$BIN" "$tname" --ignored --test-threads=1
            else
                prterun -np 1 "$BIN" "$tname" --ignored --test-threads=1
            fi
        done
        ;;
    COV)
        require_uri_daemon
        echo "Running coverage with daemon tests..."
        cargo llvm-cov --json -- --ignored --test-threads=1
        ;;
    ALL)
        require_uri_daemon
        echo "Running all daemon-dependent tests..."
        cargo test --test tool_tool_init -- --ignored --test-threads=1
        cargo test --test lib_core_api -- --ignored --test-threads=1
        cargo test --test fabric_fabric_comprehensive -- --ignored --test-threads=1
        # THREADING is opt-in when prterun is available (does not need URI file).
        if command -v prterun >/dev/null 2>&1; then
            echo "Also running THREADING suite (prterun available)..."
            "$0" THREADING
        else
            echo "Skipping THREADING suite (prterun not on PATH)."
        fi
        ;;
    *)
        echo "Usage: $0 [TOOL|LIB|FABRIC|THREADING|COV|ALL]"
        exit 1
        ;;
esac
