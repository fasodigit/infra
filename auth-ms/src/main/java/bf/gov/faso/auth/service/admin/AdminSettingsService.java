// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import bf.gov.faso.auth.infra.kafka.AdminEventProducer;
import bf.gov.faso.auth.model.AdminSetting;
import bf.gov.faso.auth.model.AdminSettingsHistory;
import bf.gov.faso.auth.repository.AdminSettingRepository;
import bf.gov.faso.auth.repository.AdminSettingsHistoryRepository;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.slf4j.MDC;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.util.List;
import java.util.Optional;
import java.util.UUID;

/**
 * Configuration Center service.
 * <p>
 * Concurrency: every {@link #update} call must include the caller's expected
 * version; if it doesn't match the row, the call throws
 * {@link OptimisticConcurrencyException} (translated to HTTP 409 by the
 * controller).
 */
@Service
public class AdminSettingsService {

    private static final Logger log = LoggerFactory.getLogger(AdminSettingsService.class);
    private static final ObjectMapper MAPPER = new ObjectMapper();

    private final AdminSettingRepository repo;
    private final AdminSettingsHistoryRepository historyRepo;
    private final AdminEventProducer eventProducer;

    public AdminSettingsService(AdminSettingRepository repo,
                                AdminSettingsHistoryRepository historyRepo,
                                AdminEventProducer eventProducer) {
        this.repo = repo;
        this.historyRepo = historyRepo;
        this.eventProducer = eventProducer;
    }

    public List<AdminSetting> getAll() {
        return repo.findAll();
    }

    public List<AdminSetting> getByCategory(String category) {
        return repo.findByCategory(category);
    }

    public Optional<AdminSetting> getByKey(String key) {
        return repo.findById(key);
    }

    /**
     * Optimistic-concurrency update. Returns the new version.
     */
    @Transactional
    public AdminSetting update(String key, String newValueJson, long expectedVersion,
                               String motif, UUID changedBy) {
        AdminSetting setting = repo.findById(key)
                .orElseThrow(() -> new IllegalArgumentException("unknown setting key: " + key));

        if (setting.getVersion() != expectedVersion) {
            throw new OptimisticConcurrencyException(
                    "version mismatch — current=" + setting.getVersion() +
                            " provided=" + expectedVersion);
        }

        validateValue(setting, newValueJson);

        String oldValueJson = setting.getValue();
        long newVersion = setting.getVersion() + 1L;

        setting.setValue(newValueJson);
        setting.setVersion(newVersion);
        setting.setUpdatedAt(Instant.now());
        setting.setUpdatedBy(changedBy);
        repo.save(setting);

        AdminSettingsHistory hist = new AdminSettingsHistory();
        hist.setKey(key);
        hist.setVersion(newVersion);
        hist.setOldValue(oldValueJson);
        hist.setNewValue(newValueJson);
        hist.setMotif(motif);
        hist.setChangedBy(changedBy);
        hist.setChangedAt(Instant.now());
        hist.setTraceId(MDC.get("traceId"));
        historyRepo.save(hist);

        eventProducer.publishSettingsChanged(key, newVersion, oldValueJson,
                newValueJson, changedBy, motif);

        log.info("Setting updated key={} version={}->{} by={}",
                key, expectedVersion, newVersion, changedBy);
        return setting;
    }

    public List<AdminSettingsHistory> getHistory(String key) {
        return historyRepo.findByKeyOrderByVersionDesc(key);
    }

    @Transactional
    public AdminSetting revert(String key, long targetVersion, String motif, UUID changedBy) {
        AdminSetting current = repo.findById(key)
                .orElseThrow(() -> new IllegalArgumentException("unknown setting key: " + key));
        AdminSettingsHistory targetHist = historyRepo.findByKeyAndVersion(key, targetVersion)
                .orElseThrow(() -> new IllegalArgumentException(
                        "history version not found: " + key + "#" + targetVersion));
        return update(key, targetHist.getNewValue(), current.getVersion(),
                "revert(v" + targetVersion + "): " + (motif == null ? "" : motif),
                changedBy);
    }

    // ── Convenience accessors used by other services ────────────────────────

    public int getInt(String key, int fallback) {
        try {
            return getByKey(key)
                    .map(s -> Integer.parseInt(stripQuotes(s.getValue())))
                    .orElse(fallback);
        } catch (Exception e) {
            return fallback;
        }
    }

    public boolean getBool(String key, boolean fallback) {
        try {
            return getByKey(key)
                    .map(s -> Boolean.parseBoolean(stripQuotes(s.getValue())))
                    .orElse(fallback);
        } catch (Exception e) {
            return fallback;
        }
    }

    public long getLong(String key, long fallback) {
        try {
            return getByKey(key)
                    .map(s -> Long.parseLong(stripQuotes(s.getValue())))
                    .orElse(fallback);
        } catch (Exception e) {
            return fallback;
        }
    }

    private String stripQuotes(String json) {
        if (json == null) return null;
        String t = json.trim();
        if (t.startsWith("\"") && t.endsWith("\"") && t.length() >= 2) {
            return t.substring(1, t.length() - 1);
        }
        return t;
    }

    private void validateValue(AdminSetting setting, String newValueJson) {
        try {
            JsonNode node = MAPPER.readTree(newValueJson);
            switch (setting.getValueType()) {
                case "INT", "LONG" -> {
                    long n = node.asLong();
                    if (setting.getMinValue() != null) {
                        long min = MAPPER.readTree(setting.getMinValue()).asLong();
                        if (n < min) throw new IllegalArgumentException("value < min " + min);
                    }
                    if (setting.getMaxValue() != null) {
                        long max = MAPPER.readTree(setting.getMaxValue()).asLong();
                        if (n > max) throw new IllegalArgumentException("value > max " + max);
                    }
                }
                case "BOOLEAN" -> {
                    if (!node.isBoolean() && !"true".equalsIgnoreCase(node.asText()) &&
                            !"false".equalsIgnoreCase(node.asText())) {
                        throw new IllegalArgumentException("not a boolean");
                    }
                }
                case "STRING", "JSON", "DOUBLE" -> { /* permissive */ }
                default -> { /* unknown type — accept */ }
            }
        } catch (IllegalArgumentException e) {
            throw e;
        } catch (Exception e) {
            throw new IllegalArgumentException("invalid value JSON: " + e.getMessage());
        }
    }

    /** HTTP 409 marker. */
    public static class OptimisticConcurrencyException extends RuntimeException {
        public OptimisticConcurrencyException(String msg) { super(msg); }
    }
}
