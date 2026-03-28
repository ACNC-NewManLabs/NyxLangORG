# Deployment Instructions

## Local Dev

1. Start registry:

```bash
cargo run --manifest-path infrastructure/registry-server/Cargo.toml
```

2. Run mirror sync:

```bash
cargo run --manifest-path infrastructure/mirror-agent/Cargo.toml -- --registry http://127.0.0.1:8090
```

3. Run build verification worker manually:

```bash
cargo run --manifest-path infrastructure/build-worker/Cargo.toml -- verify --package demo --version 1.0.0 --source-sha256 <sha256>
```

4. Start collaboration service:

```bash
cargo run --manifest-path infrastructure/collab-server/Cargo.toml
```

5. Query collaboration insights:

```bash
cargo run --manifest-path collab/nyx-collab-cli/Cargo.toml -- ecosystem-insights
```

## Docker Compose

```bash
docker compose -f infrastructure/deploy/docker-compose.yml up
```

## Kubernetes

Use `infrastructure/deploy/k8s-example.yaml` as a baseline and replace image + persistent storage config.
