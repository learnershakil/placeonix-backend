# Local Dev Infrastructure

This compose stack provisions Postgres, Redis, NATS, and MinIO for local
backend development.

## Start

```
docker compose -f infra/docker/docker-compose.yml up -d
```

## Stop

```
docker compose -f infra/docker/docker-compose.yml down
```

## Connection Details

- Control-plane Postgres: postgresql://placeonix:placeonix_dev@localhost:5432/placeonix_control
- Tenant Postgres: postgresql://placeonix:placeonix_dev@localhost:5433/placeonix_tenant
- Redis: redis://localhost:6379
- NATS: nats://localhost:4222
- MinIO endpoint: http://localhost:9000
- MinIO console: http://localhost:9001
- MinIO bucket: placeonix
