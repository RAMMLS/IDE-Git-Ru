#!/bin/bash
set -e

echo "=== Aura VCS Setup & Deploy ==="

echo "[1/2] Installing dependencies..."
# Установка зависимостей клиента
cd client
npm install
cd ..

# Сборка Rust компонентов
echo "Building Rust components..."
cargo build --release --manifest-path vcs-core/Cargo.toml
cargo build --release --manifest-path server/Cargo.toml

echo "[2/2] Starting services..."
# Запуск Rust сервера
cd server
cargo run --release &
SERVER_PID=$!
cd ..

# Запуск React клиента
cd client
npm run dev &
CLIENT_PID=$!
cd ..

echo "==================================="
echo "Aura VCS is running!"
echo "Client: http://localhost:5000"
echo "Server: http://localhost:3000"
echo "Press Ctrl+C to stop."
echo "==================================="

# Корректное завершение процессов при нажатии Ctrl+C
trap "echo 'Stopping services...'; kill $SERVER_PID $CLIENT_PID; exit" SIGINT SIGTERM

wait $SERVER_PID $CLIENT_PID
