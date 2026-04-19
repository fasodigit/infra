# ARMAGEDDON — Distributed DDoS Mitigation

SPDX-License-Identifier: AGPL-3.0-only

## Architecture overview

Local per-process rate limiting is inadequate against distributed botnets: an
attacker who spreads 1 000 req/s across 10 ARMAGEDDON replicas bypasses a
500 req/s per-process limit while appearing innocent to each individual node.

ARMAGEDDON C6 solves this with **centralized KAYA counters** shared by all
replicas:

```
client → ARMAGEDDON replica N
                    ↓
        DistributedRateLimiter::check("ip:<src>")
                    ↓  RESP3 INCR + EXPIRE
               KAYA cluster
               (atomic counter)
```

All replicas increment the same key. The counter is authoritative regardless
of which replica receives the request.

## Key schema

| Key pattern                    | Type        | Purpose                          |
|-------------------------------|-------------|----------------------------------|
| `faso:rate:ip:<addr>`         | integer     | Per-source-IP fixed window       |
| `faso:rate:asn:<number>`      | integer     | Per-ASN fixed window             |
| `faso:rate:session:<id>`      | integer     | Per-session fixed window         |
| `faso:rate:country:<cc>`      | integer     | Per-country-code fixed window    |
| `faso:swlog:ip:<addr>`        | sorted set  | Sub-second sliding window log    |

## Decisions

| Decision    | Condition                                | Action                      |
|-------------|------------------------------------------|-----------------------------|
| `Allow`     | count <= 75 % of threshold               | Forward to origin           |
| `Challenge` | 75 % < count <= 100 % of threshold       | Present CAPTCHA / slow down |
| `Block`     | count > threshold OR ASN in deny-list    | Drop connection immediately |

## Fail-open policy

KAYA round-trips are budgeted at **1 ms P99**. If KAYA is unavailable or
responds slower than `fail_open_timeout_ms` (default 1 ms), the limiter
returns `Allow` to avoid turning a cache outage into a self-inflicted DoS.

Every fail-open event is logged at `WARN` level. Operators should alert when
the rate of fail-open events exceeds 1 % of total checks.

## SYN cookie protection (kernel layer)

Before any ARMAGEDDON userspace code runs, the Linux kernel's SYN cookie
mechanism provides a first line of defence against SYN flood attacks.

### Enabling SYN cookies

```bash
# Temporary (until reboot)
sysctl -w net.ipv4.tcp_syncookies=1

# Permanent — add to /etc/sysctl.d/99-armageddon-ddos.conf
net.ipv4.tcp_syncookies = 1

# Recommended companion settings
net.ipv4.tcp_max_syn_backlog = 65536
net.ipv4.tcp_synack_retries  = 2
net.core.somaxconn            = 65535
```

Apply permanently:

```bash
sysctl --system
```

Verify:

```bash
sysctl net.ipv4.tcp_syncookies
# Expected: net.ipv4.tcp_syncookies = 1
```

### How SYN cookies work

When the SYN backlog is full, the kernel encodes connection state in the
Initial Sequence Number (ISN) of the SYN-ACK. No per-connection state is
stored until the client completes the three-way handshake with a valid ACK.
Spoofed SYNs that never complete the handshake consume zero kernel memory.

### Limitations

SYN cookies disable certain TCP options (e.g. window scaling, timestamps) on
the SYN-ACK for cookie-protected connections. This is acceptable given the
attack scenario; normal traffic through the established backlog is unaffected.

## ASN deny-list hot-reload

The file `config/asn-blocklist.yaml` is watched and reloaded every
`reload_interval_secs` seconds (default 60) without restarting the process.

```yaml
# config/asn-blocklist.yaml
asns:
  - 12345   # Example botnet AS
  - 67890   # Example bulletproof hosting AS
```

ASN lookups happen in-memory (zero KAYA round-trip) and always result in a
hard `Block` regardless of the counter value.

### Populating the deny-list

1. Cross-reference GeoIP ASN data with abuse reports:
   ```bash
   # Example: block all ASNs with > 1000 blocked requests in the last 24 h
   armageddon-admin asn-report --min-blocks 1000 --since 24h > /tmp/blocked-asns.txt
   ```
2. Update `config/asn-blocklist.yaml` with the output.
3. The hot-reload task picks up the change within `reload_interval_secs`.

## Configuration reference

```yaml
security:
  sentinel:
    distributed_rate_limit:
      enabled: true
      window_secs: 1
      threshold_per_window: 500
      challenge_ratio: 0.75       # Challenge starts at 75 % of threshold
      fail_open_timeout_ms: 1     # Fail open after 1 ms
      sliding_window_log: false   # Set true for sub-second precision
      asn_blocklist_path: /etc/armageddon/asn-blocklist.yaml
```

## Testing

```bash
# Unit tests (no KAYA required)
cargo test -p armageddon-sentinel ddos

# Integration test (requires live KAYA on 127.0.0.1:6380)
cargo test -p armageddon-sentinel -- --ignored ddos_1000_requests
```

Expected outcome for the 1 000-request integration test:
- Requests 1–500: `Allow` or `Challenge`
- Requests 501–1000: `Block`
