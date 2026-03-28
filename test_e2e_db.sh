#!/bin/bash
set -e

echo "[Test] Starting Nyx E2E Database Test..."

# 1. Start Server in background
echo "[Test] Launching NyxServer on port 9090..."
cargo run --bin nyx -- db server --port 9090 > server.log 2>&1 &
SERVER_PID=$!

# Give it time to bind
sleep 3

# 2. Run Commands via nyx-shell
echo "[Test] Connecting via nyx-shell and executing queries..."
(
  # 1. Create a table
  echo "CREATE TABLE nyx_devs (id INT, name STR, points FLOAT);"
  sleep 1
  # 2. Insert data using the new SELECT-without-FROM (Values) support
  echo "INSERT INTO nyx_devs SELECT 1 AS id, 'Antigravity' AS name, 99.9 AS points;"
  sleep 1
  echo "INSERT INTO nyx_devs SELECT 2 AS id, 'Surya' AS name, 100.0 AS points;"
  sleep 1
  # 3. Constant Select (No FROM)
  echo "SELECT 'System Operational' AS status, 2026 AS year;"
  sleep 1
  # 4. Query the table
  echo "SELECT * FROM nyx_devs;"
  sleep 1
  echo "exit"
) | cargo run --bin nyx-shell

echo "[Test] Killing server PID: $SERVER_PID"
kill $SERVER_PID || true

echo "[Test] E2E Test Complete. Checking server.log..."
cat server.log
