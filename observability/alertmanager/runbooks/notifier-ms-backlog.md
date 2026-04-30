<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 FASO DIGITALISATION
-->

# FasoKafkaConsumerLagCritical (Notifier Backlog) — Runbook

**Severity** : Critical
**Alert** : `FasoKafkaConsumerLagCritical`
**Oncall** : See `/observability/oncall.yml`
**SLA** : diagnostic + mitigation < 15 min
**Derniere mise a jour** : 2026-04-24

---

## Symptoms

- Alert `FasoKafkaConsumerLagCritical` firing with `consumer_group=notifier-consumer`
- SMS / email delivery delayed (users report not receiving OTP codes)
- Grafana dashboard "Notifier — Consumer Lag" shows growing lag
- DLQ depth increasing on `notifications.dlq` topic
- Users unable to complete OTP-based login or payment confirmation

## Impact

- **User-facing** : OTP codes delayed or missing — users locked out of login; payment confirmations not sent
- **Business** : Transaction completion rate drops; user complaints spike; regulatory risk if payment notifications fail
- **SLO** : `notification-delivery` SLO burn rate critical (target: 99% delivered within 60s)
- **Downstream** : auth-ms OTP flow blocked; poulets-api order confirmation emails delayed

## Diagnosis

### Step 1: Check consumer lag

```bash
rpk group describe notifier-consumer-group
```

Expected: `LAG` column should be 0 or near-zero. If lag > 500 on any partition, backlog is significant.

```bash
# Total lag across all partitions
rpk group describe notifier-consumer-group | awk '/^notification/{sum += $5} END {print "Total lag:", sum}'
```

### Step 2: Check DLQ depth

```bash
rpk topic consume notifications.dlq --num 10 --offset end | jq '.value | fromjson | .error_reason'
```

Common DLQ reasons:
- `SMTP connection refused` — mail server down
- `Twilio API 429` — rate limited
- `Orange SMS gateway timeout` — provider issue
- `Invalid phone number format` — data quality issue

### Step 3: Check notifier-ms health

```bash
# Pod status
kubectl get pods -l app=notifier-ms -n faso
podman ps --filter name=faso-notifier-ms

# Actuator health
curl -s http://localhost:9003/actuator/health | jq .

# Logs for errors
podman logs --tail 200 faso-notifier-ms | grep -iE "error|exception|timeout|reject"
```

### Step 4: Check SMTP connectivity

```bash
# Test SMTP (MailHog in dev, real SMTP in prod)
podman exec faso-notifier-ms curl -s telnet://mailhog:1025
# or for production
nc -zv smtp.faso.gov.bf 587
```

### Step 5: Check SMS provider status

```bash
# Twilio API status
curl -s https://status.twilio.com/api/v2/status.json | jq '.status.description'

# Orange SMS API
curl -s -o /dev/null -w "%{http_code}" https://api.orange.com/smsmessaging/v1/health

# Check rate limit headers from recent calls
podman logs --tail 500 faso-notifier-ms | grep -i "rate.limit\|429\|retry-after"
```

### Step 6: Check notifier-ms resource usage

```bash
kubectl top pod -l app=notifier-ms -n faso
podman stats --no-stream faso-notifier-ms
```

If CPU is at 100%, consumers cannot keep up with message rate.

## Remediation

### Quick Fix (< 5 min)

1. **Scale consumers** (if lag is growing but no errors):
   ```bash
   kubectl scale deploy/notifier-ms -n faso --replicas=3
   ```

2. **Switch SMS provider** (if Twilio is rate-limited or down):
   ```bash
   # Update runtime config to switch from Twilio to Orange
   curl -X POST http://localhost:9003/actuator/env \
     -H "Content-Type: application/json" \
     -d '{"name":"sms.provider","value":"orange"}'
   # Restart to pick up
   kubectl rollout restart deploy/notifier-ms -n faso
   ```

3. **Replay DLQ** (after fixing root cause):
   ```bash
   # Dry run — check what's in DLQ
   rpk topic consume notifications.dlq --num 50 | jq '.value | fromjson | .event_type' | sort | uniq -c | sort -rn

   # Replay DLQ messages back to main topic
   rpk topic consume notifications.dlq \
     --offset start --num 1000 \
     | rpk topic produce notifications.events.v1
   ```

### Root Cause Fix

- **SMTP down** : failover to backup SMTP relay; check firewall rules
- **SMS rate limit** : implement exponential backoff; distribute across multiple provider accounts
- **Consumer crash loop** : check for poison messages in topic; skip or DLQ the problematic offset
- **Memory pressure** : increase JVM heap; check for memory leaks in notification templates

### Clearing the backlog

If lag > 10000 and growing:

```bash
# Option 1: Scale aggressively (temporary)
kubectl scale deploy/notifier-ms -n faso --replicas=5

# Option 2: If messages are old (> 24h), consider skipping non-critical
# WARNING: Only skip non-OTP messages. OTPs MUST be delivered.
rpk group seek notifier-consumer-group --to end --topics notifications.marketing.v1
```

## Escalation

| Time | Action |
|------|--------|
| 0 min | Oncall acknowledges, checks consumer lag + DLQ |
| 5 min | If OTP delivery affected, escalate immediately to SRE lead |
| 15 min | If not resolved, page engineering manager |
| 30 min | If SMS provider issue, contact provider support (Twilio / Orange) |
| 60 min | Incident commander activated; consider public status page update |

## Prevention

- Monitor consumer lag with alert threshold: WARNING at lag > 100, CRITICAL at lag > 500
- Set up dual SMS provider (Twilio + Orange) with automatic failover
- Rate limit notification production at source (poulets-api) to prevent flood
- Implement circuit breaker on SMS/SMTP calls with fallback to retry queue
- Monthly chaos test: kill one notifier instance during peak, verify lag recovery

## Related Alerts

- `FasoRedpandaPartitionOffline` — Redpanda issues cause consumer lag
- `FasoAuthMsDown` — OTP delivery depends on notifier-ms
- `OutboxDeadLetter` — outbox relay may produce to notification topics
- `FasoSvidExpiryCritical` — mTLS failure blocks notifier-ms from reaching Redpanda
