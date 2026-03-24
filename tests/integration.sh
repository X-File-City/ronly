#!/bin/bash
set -euo pipefail

# Integration tests for ronly.
# Must run as root (needs CAP_SYS_ADMIN for namespaces).

if [ "$(id -u)" -ne 0 ]; then
  echo "error: must run as root" >&2
  exit 1
fi

RONLY="${RONLY_BIN:-./target/release/ronly}"
if [ ! -x "$RONLY" ]; then
  echo "error: $RONLY not found" >&2
  exit 1
fi

PASS=0
FAIL=0

# Run: ronly -- args...
# Check exit code
run_test() {
  local name="$1" expected_rc="$2"
  shift 2
  local output rc
  output=$("$RONLY" -- "$@" 2>&1) && rc=0 || rc=$?
  if [ "$rc" -eq "$expected_rc" ]; then
    echo "  ok  $name"
    PASS=$((PASS + 1))
  else
    echo "FAIL  $name (rc=$rc, want $expected_rc)"
    echo "      $output"
    FAIL=$((FAIL + 1))
  fi
}

# Run: ronly -- args...
# Check exit code + grep output
run_test_grep() {
  local name="$1" expected_rc="$2" pattern="$3"
  shift 3
  local output rc
  output=$("$RONLY" -- "$@" 2>&1) && rc=0 || rc=$?
  if [ "$rc" -eq "$expected_rc" ] && \
     echo "$output" | grep -qi "$pattern"; then
    echo "  ok  $name"
    PASS=$((PASS + 1))
  else
    echo "FAIL  $name (rc=$rc, want $expected_rc, grep=$pattern)"
    echo "      $output"
    FAIL=$((FAIL + 1))
  fi
}

echo "--- read operations ---"
run_test_grep "echo hello" 0 "hello" \
  echo hello
run_test "cat /etc/hostname" 0 \
  cat /etc/hostname
run_test "ls /" 0 \
  ls /
run_test "ps aux" 0 \
  bash -c "ps aux | head -3"

echo "--- write operations blocked ---"
run_test_grep "rm blocked" 1 \
  "read-only\|not permitted" \
  bash -c "rm /etc/hostname 2>&1"
run_test "touch blocked" 1 \
  bash -c "touch /etc/ronly_test 2>&1"
run_test "mkdir blocked" 1 \
  bash -c "mkdir /etc/ronly_test 2>&1"

echo "--- /tmp writable ---"
run_test "/tmp write+read" 0 \
  bash -c "echo test > /tmp/ronly_test && cat /tmp/ronly_test"

echo "--- pid namespace ---"
run_test_grep "ps shows host init" 0 \
  "init\|systemd" \
  ps -p 1 -o comm=
run_test_grep "own pid is 1" 0 "^1$" \
  bash -c 'echo $$'

echo "--- seccomp ---"
run_test_grep "kill blocked" 1 \
  "not permitted" \
  bash -c "kill 1 2>&1"

echo "--- shims ---"
run_test_grep "docker exec blocked" 1 \
  "blocked" \
  bash -c "docker exec foo bar 2>&1"
run_test_grep "docker stop blocked" 1 \
  "blocked" \
  bash -c "docker stop foo 2>&1"
run_test_grep "kubectl delete blocked" 1 \
  "blocked" \
  bash -c "kubectl delete pod foo 2>&1"
run_test_grep "kubectl apply blocked" 1 \
  "blocked" \
  bash -c "kubectl apply -f foo 2>&1"

echo "--- exit codes ---"
run_test "exit 0" 0 true
run_test "exit 1" 1 false
run_test "exit 42" 42 bash -c "exit 42"

echo ""
echo "$PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
