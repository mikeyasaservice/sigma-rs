# Sigma-rs Deployment Guide

This directory contains deployment configurations for sigma-rs.

## Directory Structure

```
deployment/
├── k8s/                    # Kubernetes manifests
│   ├── base/              # Base configuration
│   └── overlays/          # Environment-specific overlays
│       ├── development/
│       ├── staging/
│       └── production/
└── helm/                   # Helm chart
    └── sigma-rs/
```

## Quick Start

### Using Kubernetes Manifests

```bash
# Deploy to development
kubectl apply -k deployment/k8s/overlays/development

# Deploy to production
kubectl apply -k deployment/k8s/overlays/production

# Or use kubectl directly
kubectl apply -f deployment/k8s/base/
```

### Using Helm

```bash
# Add Helm repository (if published)
helm repo add sigma-rs https://charts.sigma-rs.io
helm repo update

# Install with default values
helm install sigma-rs deployment/helm/sigma-rs

# Install with custom values
helm install sigma-rs deployment/helm/sigma-rs \
  --set kafka.brokers="kafka-0:9092,kafka-1:9092" \
  --set replicaCount=5

# Install with values file
helm install sigma-rs deployment/helm/sigma-rs \
  -f my-values.yaml
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Log level | `info,sigma_rs=debug` |
| `SIGMA_RULES_DIR` | Rules directory | `/rules` |
| `SIGMA_WORKER_THREADS` | Worker thread count | `4` |
| `KAFKA_BROKERS` | Kafka broker list | `localhost:9092` |
| `KAFKA_GROUP_ID` | Consumer group ID | `sigma-rs` |
| `KAFKA_TOPICS` | Topics to consume | `security-events` |

### Resource Requirements

#### Minimum (Development)
- CPU: 500m
- Memory: 512Mi

#### Recommended (Production)
- CPU: 4 cores
- Memory: 8Gi

#### High Performance
- CPU: 16+ cores
- Memory: 32Gi+

## Monitoring

### Prometheus Metrics

The application exposes metrics on port 9090 at `/metrics`.

Key metrics:
- `sigma_events_processed_total`
- `sigma_rule_evaluation_duration_seconds`
- `sigma_kafka_consumer_lag`
- `sigma_errors_total`

### Grafana Dashboard

Import the dashboard from `deployment/monitoring/grafana-dashboard.json`.

### Alerts

Prometheus alert rules are included in the Helm chart:
- High consumer lag (> 100K messages)
- High error rate (> 1%)
- High memory usage (> 90%)

## Security

### RBAC

The deployment includes minimal RBAC permissions:
- Read ConfigMaps (for rules)
- Read Pods (for self-monitoring)

### Network Policies

Example network policy for production:

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: sigma-rs
spec:
  podSelector:
    matchLabels:
      app: sigma-rs
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: monitoring
    ports:
    - protocol: TCP
      port: 9090
  egress:
  - to:
    - namespaceSelector:
        matchLabels:
          name: kafka
    ports:
    - protocol: TCP
      port: 9092
```

## Scaling

### Horizontal Scaling

The HPA automatically scales based on:
- CPU utilization
- Memory utilization
- Kafka consumer lag

### Vertical Scaling

Adjust resource requests/limits in the deployment:

```yaml
resources:
  requests:
    cpu: "8"
    memory: "16Gi"
  limits:
    cpu: "16"
    memory: "32Gi"
```

## Troubleshooting

### Pod Not Starting

```bash
# Check logs
kubectl logs -l app=sigma-rs

# Check events
kubectl describe pod -l app=sigma-rs

# Check resource usage
kubectl top pod -l app=sigma-rs
```

### High Memory Usage

1. Enable string interning
2. Reduce batch size
3. Check for memory leaks in rules

### Kafka Connection Issues

```bash
# Test connectivity
kubectl exec -it deployment/sigma-rs -- nc -zv kafka-0.kafka 9092

# Check consumer group
kubectl exec -it deployment/sigma-rs -- \
  kafka-consumer-groups --bootstrap-server kafka:9092 \
  --group sigma-rs --describe
```

## Production Checklist

- [ ] Configure appropriate resource limits
- [ ] Enable autoscaling (HPA)
- [ ] Set up monitoring and alerts
- [ ] Configure pod disruption budget
- [ ] Enable OpenTelemetry tracing
- [ ] Mount rules from external source
- [ ] Configure Kafka authentication
- [ ] Set up backup strategy for offsets
- [ ] Test disaster recovery procedures
- [ ] Document runbooks

## Best Practices

1. **Use GitOps**: Store configurations in Git and use ArgoCD/Flux
2. **Separate Rules**: Mount rules from ConfigMap/PVC, not in image
3. **Monitor Lag**: Keep consumer lag under control
4. **Regular Updates**: Update rules without restarting pods
5. **Gradual Rollouts**: Use rolling updates with proper health checks