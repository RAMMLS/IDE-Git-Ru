#!/bin/bash
set -e

echo "Starting Aura VCS..."

# Start the Rust server
cd server
cargo run &
SERVER_PID=$!
cd ..

# Start the React client
cd client
npm run dev &
CLIENT_PID=$!
cd ..

echo "Aura VCS is running."
echo "Server PID: $SERVER_PID"
echo "Client PID: $CLIENT_PID"
echo "Press Ctrl+C to stop."

wait $SERVER_PID $CLIENT_PID
