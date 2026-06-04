# DrafftInk deployment

## Single-replica relay (PVC)

```bash
kubectl apply -f k8s/relay-deployment.yaml
```

Set `STORE=file` and mount a PVC at `PERSISTENCE_DIR`. Room snapshots survive pod restarts.

## Same-origin ingress

`k8s/ingress.yaml` routes `/ws` to the relay and `/` to your static WASM service. Enable WebSocket timeouts on your ingress controller.

## Multi-replica (optional Redis)

```bash
cargo build --release -p drafftink-server --features redis
kubectl apply -f k8s/redis-relay.yaml
```

Do not run `replicas > 1` with file storage only; use Redis pub/sub + snapshot keys.
