# Placeonix Backend Operations Runbook

## Release Gates

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `cargo build --all-features`
- Apply control-plane migrations before tenant migrations.
- Confirm `JWT_SECRET` is at least 32 bytes and different per environment.

## Exam Window Readiness

- Confirm Postgres backups and restore drills are current.
- Confirm Redis memory headroom and stream pending entries are healthy.
- Check API p95 latency, DB pool saturation, judge queue depth, autosave errors, and proctor event ingestion rate.
- Set queue backpressure alert at `QUEUE_MAX_DEPTH`.

## Incident Response

- If autosave errors rise, keep attempts resumable and prioritize `/api/v1/attempts/:id/answers`.
- If judge depth exceeds threshold, return queued/system-busy states instead of holding API sockets.
- If proctor event ingestion slows, preserve append-only events first and defer scoring workers.
- Audit all evidence access and proctor decisions during disputes.

## Recovery

- Restore control-plane DB first, then tenant DBs.
- Recreate Redis Streams from `platform.job_outbox` for unpublished jobs.
- Replay `platform.realtime_outbox` only for events still relevant to active sessions.
