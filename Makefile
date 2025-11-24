SHELL := /bin/bash

CARGO ?= cargo
COMPOSE ?= docker compose
DOCKER_TAG ?= rafka:latest

BROKER_ARGS ?= --port 50051 --partition 0 --total-partitions 1
CONSUMER_ARGS ?= --port 50051
PRODUCER_ARGS ?= --message "Hello, Rafka!" --key "key-1"
METRICS_ARGS ?= --port 9092
CHECK_ARGS ?= --port 50051
BENCH_ARGS ?=

.DEFAULT_GOAL := help

.PHONY: help build release check fmt fmt-check clippy test clean \
	run-broker run-consumer run-producer metrics-server check-metrics benchmark kill \
	demo-helloworld demo-partitioned demo-retention demo-offset demo-storage demo-cluster demo-p2p demo-performance \
	verify-persistence verify-p2p consumer-groups \
	docker-build compose-up compose-down compose-logs \
	k8s-deploy k8s-delete k8s-status \
	publish-prepare publish-all publish-restore

help: ## Show this help message
	@echo "Available targets:"
	@grep -Eh '^[a-zA-Z0-9_.-]+:.*##' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*## "}; {printf "  %-24s %s\n", $$1, $$2}'

build: ## Build debug binaries
	$(CARGO) build

release: ## Build release binaries
	$(CARGO) build --release

check: ## Type-check all crates
	$(CARGO) check --workspace --all-targets

fmt: ## Format the workspace
	$(CARGO) fmt

fmt-check: ## Check formatting without writing changes
	$(CARGO) fmt -- --check

clippy: ## Run Clippy linting
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings

test: ## Run all tests
	$(CARGO) test --workspace --all-targets

clean: ## Remove build artifacts
	$(CARGO) clean

run-broker: ## Run broker (override BROKER_ARGS="--port 50051 ...")
	$(CARGO) run --bin start_broker -- $(BROKER_ARGS)

run-consumer: ## Run consumer (override CONSUMER_ARGS)
	$(CARGO) run --bin start_consumer -- $(CONSUMER_ARGS)

run-producer: ## Run producer (override PRODUCER_ARGS)
	$(CARGO) run --bin start_producer -- $(PRODUCER_ARGS)

metrics-server: ## Run metrics server (override METRICS_ARGS)
	$(CARGO) run --bin metrics_server -- $(METRICS_ARGS)

check-metrics: ## Query metrics via check_metrics (override CHECK_ARGS)
	$(CARGO) run --bin check_metrics -- $(CHECK_ARGS)

benchmark: ## Run benchmark binary (override BENCH_ARGS)
	$(CARGO) run --bin benchmark -- $(BENCH_ARGS)

kill: ## Stop running broker/consumer/producer processes
	bash scripts/kill.sh

demo-helloworld: ## Run basic broker/producer/consumer demo
	bash scripts/helloworld.sh

demo-partitioned: ## Run multi-partition demo
	bash scripts/partitioned_demo.sh

demo-retention: ## Run retention policy demo
	bash scripts/retention_demo.sh

demo-offset: ## Run offset tracking demo
	bash scripts/offset_tracking_demo.sh

demo-storage: ## Run storage demo with consumer restart
	bash scripts/storage_demo.sh

demo-cluster: ## Run cluster demo with multiple brokers
	bash scripts/cluster_demo.sh

demo-p2p: ## Run P2P mesh networking demo
	bash scripts/p2p_mesh_demo.sh

demo-performance: ## Run performance and metrics demo
	bash scripts/performance_demo.sh

verify-persistence: ## Verify WAL persistence survives broker restart
	bash scripts/verify_persistence.sh

verify-p2p: ## Verify P2P connectivity between brokers
	bash scripts/verify_p2p.sh

consumer-groups: ## Run consumer group distribution test
	bash scripts/test_consumer_groups.sh

docker-build: ## Build Docker image
	docker build -t $(DOCKER_TAG) .

compose-up: ## Start Docker Compose cluster
	$(COMPOSE) up --build -d

compose-down: ## Stop Docker Compose cluster
	$(COMPOSE) down

compose-logs: ## Tail Docker Compose logs
	$(COMPOSE) logs -f

k8s-deploy: ## Deploy to Kubernetes using provided manifest
	bash scripts/deploy-k8s.sh

k8s-delete: ## Remove Kubernetes resources
	kubectl delete -f k8s/rafka-deployment.yaml

k8s-status: ## Show Kubernetes status for Rafka namespace
	kubectl get pods -n rafka && kubectl get services -n rafka

publish-prepare: ## Swap to version dependencies for publishing
	bash scripts/prepare_for_publish.sh prepare

publish-all: ## Publish crates to crates.io in dependency order
	bash scripts/prepare_for_publish.sh publish

publish-restore: ## Restore local path dependencies after publishing
	bash scripts/prepare_for_publish.sh restore
