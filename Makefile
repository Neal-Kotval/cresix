.PHONY: dev cloud-dev test build up down doctor team-qa team-smoke

dev:
	cargo run -p c6-server

cloud-dev:
	cargo run -p c6-cloud

test:
	cargo test --workspace
	cd web && npm test
	cd cloud-web && npm test

build:
	cargo build --workspace
	cd web && npm run build
	cd cloud-web && npm run build

up:
	docker compose up --build -d

down:
	docker compose down

doctor:
	@curl --fail --silent http://127.0.0.1:$${C6_PORT:-8787}/healthz
	@echo " C6 is healthy"

team-qa:
	bash teams/c6-build-team/qa/all.sh

team-smoke:
	bash teams/c6-build-team/qa/smoke.sh
