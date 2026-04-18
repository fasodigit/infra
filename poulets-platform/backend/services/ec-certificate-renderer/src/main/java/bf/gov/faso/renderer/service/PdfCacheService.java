package bf.gov.faso.renderer.service;

import bf.gov.faso.renderer.config.RendererProperties;
import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.SerializationFeature;
import com.github.benmanes.caffeine.cache.Cache;
import com.github.benmanes.caffeine.cache.Caffeine;
import com.github.benmanes.caffeine.cache.stats.CacheStats;
import jakarta.annotation.PostConstruct;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.time.Duration;
import java.util.HexFormat;
import java.util.Map;
import java.util.Optional;

@Service
public class PdfCacheService {

    private static final Logger log = LoggerFactory.getLogger(PdfCacheService.class);

    private final RendererProperties props;

    private final ObjectMapper stableMapper = new ObjectMapper()
            .configure(SerializationFeature.ORDER_MAP_ENTRIES_BY_KEYS, true);

    private Cache<String, byte[]> cache;
    private boolean enabled;

    public PdfCacheService(RendererProperties props) {
        this.props = props;
    }

    @PostConstruct
    public void init() {
        this.enabled = props.cacheEnabled();

        if (!enabled) {
            log.info("PDF cache disabled (renderer.cache-enabled=false)");
            return;
        }

        this.cache = Caffeine.newBuilder()
                .maximumSize(props.cacheMaxSize())
                .expireAfterWrite(Duration.ofSeconds(props.cacheTtlSeconds()))
                .recordStats()
                .build();

        log.info("PDF cache initialized — maxSize={}, TTL={}s",
                props.cacheMaxSize(), props.cacheTtlSeconds());
    }

    public Optional<byte[]> get(String templateName, Map<String, Object> data) {
        if (!enabled || cache == null) return Optional.empty();

        String key = buildKey(templateName, data);
        byte[] cached = cache.getIfPresent(key);

        if (cached != null) {
            log.debug("Cache HIT — template={}, {} bytes", templateName, cached.length);
        }

        return Optional.ofNullable(cached);
    }

    public void put(String templateName, Map<String, Object> data, byte[] pdf) {
        if (!enabled || cache == null) return;

        String key = buildKey(templateName, data);
        cache.put(key, pdf);
        log.debug("Cache PUT — template={}, {} bytes, key={}", templateName, pdf.length,
                key.substring(0, 12) + "...");
    }

    public void invalidateTemplate(String templateName) {
        if (!enabled || cache == null) return;
        cache.invalidateAll();
        log.info("Cache invalidated for template: {}", templateName);
    }

    public void clear() {
        if (cache != null) cache.invalidateAll();
    }

    public CacheStats stats() {
        if (!enabled || cache == null) return CacheStats.empty();
        return cache.stats();
    }

    public long estimatedSize() {
        if (!enabled || cache == null) return 0;
        return cache.estimatedSize();
    }

    public boolean isEnabled() { return enabled; }

    private String buildKey(String templateName, Map<String, Object> data) {
        try {
            String json = stableMapper.writeValueAsString(data);
            String raw  = templateName + ":" + json;

            MessageDigest digest = MessageDigest.getInstance("SHA-256");
            byte[] hash = digest.digest(raw.getBytes(StandardCharsets.UTF_8));
            return HexFormat.of().formatHex(hash);

        } catch (JsonProcessingException | NoSuchAlgorithmException e) {
            log.warn("Error computing cache key: {}", e.getMessage());
            return templateName + ":" + data.hashCode();
        }
    }
}
