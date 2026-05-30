.PHONY: setup build test dev server web cli ide e2e clean docker-up docker-down

AURA_STORAGE_ROOT ?= $(CURDIR)/.aura-server
AURA_API_PORT ?= 3000
AURA_WEB_PORT ?= 5173

setup:
	npm --prefix web install

build:
	cargo build --workspace --release
	npm --prefix web run build

test:
	cargo test --workspace
	npm --prefix web run build

dev:
	./run.sh

server:
	AURA_STORAGE_ROOT="$(AURA_STORAGE_ROOT)" cargo run -p aura-server

web:
	VITE_AURA_API_URL="http://localhost:$(AURA_API_PORT)/api" npm --prefix web run dev -- --host 0.0.0.0 --port $(AURA_WEB_PORT)

cli:
	cargo run -p aura-control --bin aura -- $(ARGS)

ide:
	AURA_BIN="$(CURDIR)/target/debug/aura" cargo run --manifest-path ide/Cargo.toml

e2e:
	node test-e2e.mjs

docker-up:
	docker compose up --build

docker-down:
	docker compose down

clean:
	cargo clean
	rm -rf web/dist
