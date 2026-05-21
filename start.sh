#!/bin/bash

# Скрипт для одновременного запуска сервера и фронтенда Aura

# Завершаем оба процесса при остановке скрипта (Ctrl+C)
trap 'echo "\nОстановка сервисов..."; kill $BACKEND_PID $FRONTEND_PID 2>/dev/null; exit' SIGINT SIGTERM

# Проверяем, передан ли путь к репозиторию, иначе используем текущую директорию
REPO_PATH=${1:-.}
export AURA_REPO_PATH=$REPO_PATH

echo "🚀 Запуск Aura VCS..."
echo "📂 Репозиторий: $REPO_PATH"

# Запуск Backend (Rust)
echo "📦 Запуск backend сервера на порту 3000..."
cd server
cargo run &
BACKEND_PID=$!
cd ..

# Небольшая пауза, чтобы бекенд успел стартовать
sleep 2

# Запуск Frontend (Vite)
echo "🎨 Запуск frontend интерфейса на порту 5000..."
cd web
npm run dev &
FRONTEND_PID=$!
cd ..

echo "✅ Все сервисы запущены!"
echo "   Backend API: http://localhost:3000"
echo "   Frontend UI: http://localhost:5000"
echo "   (Нажмите Ctrl+C для остановки)"

# Ожидание завершения процессов
wait
