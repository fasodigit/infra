# WAL Replay Nightly — Runbook

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## Objectif

Valider chaque nuit que la chaîne **WAL append → snapshot → crash → recovery** réconcilie fidèlement l'état du Store.
Toute divergence déclenche une issue GitHub P1 (corruption silencieuse = incident critique).

## Exécution locale

```bash
cd INFRA/kaya
KAYA_FSYNC_POLICY=everysec cargo test --release --test wal_replay_nightly -- --nocapture
```

Variable : `KAYA_FSYNC_POLICY ∈ {always, everysec, no}`.

## Investigation d'une divergence

1. Télécharger l'artefact `replay.log` du workflow GH Actions en échec.
2. Identifier les clés divergentes (jusqu'à 5 premières loggées sur stderr).
3. Inspecter le snapshot + WAL via CLI :
   ```bash
   cargo run -p kaya-cli -- persistence inspect --data-dir <path>
   ```
4. Vérifier CRC xxh3_64 des WAL segments et du snapshot header.
5. Cas courants :
   - **Truncation tolérée** (< 64 KB) — comportement attendu, pas une régression.
   - **CRC mismatch sur segment complet** — bug WAL append (fsync pas assez fort ? race condition ?).
   - **Snapshot corrompu** — bug `SnapshotWriter` compression Zstd (vérifier `max_decompressed_size`).

## Procédure restauration prod

En cas de corruption prod détectée :
1. Arrêter `kaya-server` sur la réplique affectée.
2. Copier snapshot + WAL sur machine diagnostic.
3. Lancer `cargo run -p kaya-cli -- persistence recover --data-dir <path> --dry-run`.
4. Si OK : rsync depuis réplique saine + `recover --apply`.
5. Si KO : réinitialiser réplique depuis snapshot le plus récent d'une autre réplique.

## Metrics publiées

- `kaya_wal_replay_success{fsync=always|everysec}` → 0/1 (Pushgateway)
- Alerts Prometheus : burn > 1 en 7j consécutifs → page SRE

## Coverage actuel

- [x] SET round-trip
- [x] DEL round-trip
- [ ] SADD / ZADD / HSET (TODO Vague 2)
- [ ] Persistence probabilistic structures (Cuckoo/HLL/CMS/TopK) — dépend intégration V3 pending
- [ ] Mixed fsync mode (`always` pendant writes critiques, `everysec` sinon)
