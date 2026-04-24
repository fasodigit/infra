<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 FASO DIGITALISATION
-->

# FasoRedpandaPartitionOffline — Runbook

**Severity** : Critical
**Alert** : `FasoRedpandaPartitionOffline`
**Oncall** : See `/observability/oncall.yml`
**SLA** : diagnostic + mitigation < 15 min
**Derniere mise a jour** : 2026-04-24

---

## Symptoms

- Alert `FasoRedpandaPartitionOffline` firing in Alertmanager
- Producers receiving `NotLeaderForPartition` or `UnknownTopicOrPartition` errors
- Consumer lag spiking across all consumer groups
- Outbox relay failing to publish events (see `runbook-outbox-dlq.md`)
- notifier-ms consumer stalled on affected partitions
- Redpanda Console showing partitions without a leader

## Impact

- **User-facing** : Event-driven flows stalled — order confirmations delayed, notifications queued, outbox events stuck
- **Business** : Asynchronous operations (SMS, email, payment confirmation, state transitions) are blocked
- **SLO** : `event-delivery` SLO burn rate critical; cascading to `notification-delivery` SLO
- **Data risk** : If producer retries exhaust, messages may be lost (depends on client `acks` setting)

## Diagnosis

### Step 1: Check cluster health

```bash
rpk cluster health
```

Expected: All brokers `alive`, all partitions have a leader. Key output:
- `HEALTHY` — no offline partitions
- `UNHEALTHY` — one or more partitions without a leader

### Step 2: Identify offline partitions

```bash
rpk cluster health | grep -i "leaderless\|offline\|under-replicated"

# Detailed partition info for all topics
rpk topic list --detailed
```

For a specific topic:

```bash
rpk topic describe poulets.orders.v1
rpk topic describe notifications.events.v1
rpk topic describe outbox.events.v1
```

Check `LEADER` column: `-1` means no leader elected.

### Step 3: Check broker status

```bash
rpk cluster info
rpk redpanda admin brokers list

# Broker logs
podman logs --tail 200 faso-redpanda | grep -iE "error|panic|raft|partition|offline"

# Kubernetes
kubectl get pods -l app=redpanda -n redpanda
kubectl logs -l app=redpanda -n redpanda --tail=200 | grep -iE "error|panic|raft"
```

### Step 4: Check disk usage per broker

```bash
# Disk space
podman exec faso-redpanda df -h /var/lib/redpanda/data
podman exec faso-redpanda du -sh /var/lib/redpanda/data/kafka/

# Per-topic disk usage
podman exec faso-redpanda du -sh /var/lib/redpanda/data/kafka/*/ | sort -hr | head -20
```

If disk > 85%, Redpanda may refuse writes and partition leaders may step down.

### Step 5: Check Raft group health

```bash
rpk redpanda admin partitions list --status nok
```

This lists partitions where Raft consensus is broken (not enough replicas in ISR).

### Step 6: Check network connectivity between brokers

```bash
# From broker 0 to broker 1
podman exec faso-redpanda curl -s telnet://redpanda-1:33145
# Internal RPC port: 33145
```

## Remediation

### Quick Fix (< 5 min)

1. **Restart the failing broker** :
   ```bash
   # Identify which broker is down
   rpk cluster info | grep -v alive

   # Restart it
   podman restart faso-redpanda  # single-node dev

   # Kubernetes
   kubectl delete pod redpanda-<N> -n redpanda
   # Pod will be recreated by StatefulSet
   ```

2. **Wait for leader election** (usually < 30s after broker restart):
   ```bash
   # Poll until healthy
   for i in $(seq 1 30); do
     STATUS=$(rpk cluster health 2>&1)
     echo "$STATUS" | grep -q "HEALTHY" && echo "Cluster healthy" && break
     echo "Waiting... ($i/30)"
     sleep 2
   done
   ```

3. **Force leader election** (if automatic election is stuck):
   ```bash
   rpk redpanda admin partitions transfer-leadership \
     --namespace kafka --topic poulets.orders.v1 --partition 0 --target-broker 0
   ```

### Root Cause Fix

- **Disk full** :
  ```bash
  # Reduce retention on non-critical topics
  rpk topic alter-config notifications.marketing.v1 --set retention.ms=86400000  # 1 day

  # Delete old segments manually (DANGEROUS — only for non-critical topics)
  rpk topic trim-prefix notifications.marketing.v1 --offset 1000000
  ```

- **Broker crash loop** : Check for corrupted data segments:
  ```bash
  podman logs faso-redpanda | grep -i "corrupt\|checksum\|bad.*segment"
  # May need to delete corrupted segment and let replication rebuild
  ```

- **Network partition** : Fix network between brokers, then restart affected broker

- **Increase replication factor** (for critical topics):
  ```bash
  rpk topic alter-config poulets.orders.v1 --set replication.factor=3
  rpk topic alter-config notifications.events.v1 --set replication.factor=3
  ```

### Post-recovery: Verify consumer groups

```bash
# Check all consumer groups are caught up
rpk group list
rpk group describe notifier-consumer-group
rpk group describe outbox-relay-group
rpk group describe poulets-consumer-group
```

If any group is lagging, consumers will catch up automatically. Monitor until lag reaches 0.

## Escalation

| Time | Action |
|------|--------|
| 0 min | Oncall acknowledges, checks cluster health |
| 5 min | If broker won't restart, escalate to platform team |
| 15 min | If data corruption suspected, escalate to SRE lead |
| 30 min | If not resolved, page engineering manager |
| 60 min | Incident commander activated; assess data loss impact |

## Prevention

- Minimum 3 brokers with replication factor 3 for critical topics
- Monitor disk usage with alert at 70% (`redpanda_storage_disk_usage_percentage > 70`)
- Set topic retention policies appropriate to each topic's criticality
- Anti-affinity rules for broker pods across availability zones
- Regular backup of topic configurations (`rpk topic list --detailed > backup.yaml`)
- Chaos test: kill one broker monthly, verify automatic recovery

## Related Alerts

- `FasoKafkaConsumerLagCritical` — consumer lag spikes when partitions are offline
- `OutboxDeadLetter` — outbox relay fails to publish to offline partitions
- `FasoNotifierBacklog` — notifier consumer stalls on affected partitions
- `FasoRedpandaDiskFull` — disk full causes partition leader step-down
