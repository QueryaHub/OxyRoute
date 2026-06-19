#!/bin/bash
uv run granian --interface rsgi perf-test.test_sqlx:app --workers 1 --port 8000 &
PID=$!
sleep 2
echo "Testing SQLite /test_db endpoint:"
curl -s http://127.0.0.1:8000/test_db
echo ""
kill $PID
