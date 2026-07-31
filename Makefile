.PHONY: dev test build up down doctor

dev:
	cargo run -p c6-server

test:
	cargo test --workspace
	cd web && npm test

build:
	cargo build --workspace
	cd web && npm run build

up:
	docker compose up --build -d

down:
	docker compose down

doctor:
	@curl --fail --silent http://127.0.0.1:$${C6_PORT:-8787}/healthz
	@echo " C6 is healthy"
