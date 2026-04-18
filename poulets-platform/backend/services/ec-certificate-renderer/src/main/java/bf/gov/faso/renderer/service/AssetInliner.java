package bf.gov.faso.renderer.service;

import jakarta.annotation.PostConstruct;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;

import java.io.IOException;
import java.io.InputStream;
import java.util.Base64;
import java.util.Collections;
import java.util.HashMap;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

@Component
public class AssetInliner {

    private static final Logger log = LoggerFactory.getLogger(AssetInliner.class);

    private Map<String, String> commonAssets = Collections.emptyMap();
    private final Map<String, Map<String, String>> templateAssets = new ConcurrentHashMap<>();

    private static final Set<String> KNOWN_TEMPLATES = Set.of(
            "ACTE_NAISSANCE",
            "ACTE_MARIAGE",
            "ACTE_DECES",
            "PERMIS_PORT_ARMES",
            "ACTE_DIVERS"
    );

    @PostConstruct
    public void loadAll() {
        log.info("=== AssetInliner: loading static resources ===");
        long start = System.currentTimeMillis();

        commonAssets = loadCommonAssets();

        for (String template : KNOWN_TEMPLATES) {
            Map<String, String> specific = loadTemplateAssets(template);
            if (!specific.isEmpty()) {
                templateAssets.put(template, specific);
                log.info("  [{}] {} specific asset(s) loaded", template, specific.size());
            }
        }

        long elapsed = System.currentTimeMillis() - start;
        long totalKb  = estimateSizeKb();
        log.info("=== AssetInliner: {} common, {} templates, ~{} KB in memory ({}ms) ===",
                commonAssets.size(), templateAssets.size(), totalKb, elapsed);
    }

    public Map<String, String> getAssetsForTemplate(String templateName) {
        Map<String, String> merged = new HashMap<>(commonAssets);
        Map<String, String> specific = templateAssets.get(templateName);
        if (specific != null) {
            merged.putAll(specific);
        }
        return Collections.unmodifiableMap(merged);
    }

    public Map<String, String> getCommonAssets() {
        return Collections.unmodifiableMap(commonAssets);
    }

    public int totalAssetCount() {
        int specific = templateAssets.values().stream().mapToInt(Map::size).sum();
        return commonAssets.size() + specific;
    }

    private Map<String, String> loadCommonAssets() {
        Map<String, String> assets = new HashMap<>();

        loadAsset("assets/common/sceau-bf.png",           "image/png")
                .ifPresent(uri -> assets.put("sealUri", uri));
        loadAsset("assets/common/logo-burkina.png",        "image/png")
                .ifPresent(uri -> assets.put("logoUri", uri));
        loadAsset("assets/common/armoiries-bf.png",        "image/png")
                .ifPresent(uri -> assets.put("armoiriesUri", uri));
        loadAsset("assets/common/Marianne-Regular.woff2",  "font/woff2")
                .ifPresent(uri -> assets.put("fontMarianne", uri));
        loadAsset("assets/common/Marianne-Bold.woff2",     "font/woff2")
                .ifPresent(uri -> assets.put("fontMarianneB", uri));
        loadAsset("assets/common/Marianne-Light.woff2",    "font/woff2")
                .ifPresent(uri -> assets.put("fontMarianneL", uri));
        loadAsset("assets/common/DejaVuSans.woff2",        "font/woff2")
                .ifPresent(uri -> assets.put("fontFallback", uri));

        log.info("  [common] {} asset(s) loaded", assets.size());
        return Collections.unmodifiableMap(assets);
    }

    private Map<String, String> loadTemplateAssets(String template) {
        Map<String, String> assets = new HashMap<>();
        String base = "assets/" + template + "/";

        return switch (template) {

            case "ACTE_NAISSANCE" -> {
                loadAsset(base + "filigrane.png",        "image/png")
                        .ifPresent(uri -> assets.put("filigraneUri", uri));
                loadAsset(base + "entete-naissance.png", "image/png")
                        .ifPresent(uri -> assets.put("enteteUri", uri));
                yield assets;
            }

            case "ACTE_MARIAGE" -> {
                loadAsset(base + "filigrane.png",        "image/png")
                        .ifPresent(uri -> assets.put("filigraneUri", uri));
                loadAsset(base + "entete-mariage.png",   "image/png")
                        .ifPresent(uri -> assets.put("enteteUri", uri));
                yield assets;
            }

            case "ACTE_DECES" -> {
                loadAsset(base + "filigrane.png",     "image/png")
                        .ifPresent(uri -> assets.put("filigraneUri", uri));
                loadAsset(base + "entete-deces.png",  "image/png")
                        .ifPresent(uri -> assets.put("enteteUri", uri));
                yield assets;
            }

            case "PERMIS_PORT_ARMES" -> {
                loadAsset(base + "filigrane-armes.png", "image/png")
                        .ifPresent(uri -> assets.put("filigraneUri", uri));
                loadAsset(base + "armoiries-bf.png",    "image/png")
                        .ifPresent(uri -> assets.put("armoiriesUri", uri));
                loadAsset(base + "entete-armes.png",    "image/png")
                        .ifPresent(uri -> assets.put("enteteUri", uri));
                yield assets;
            }

            case "ACTE_DIVERS" -> {
                loadAsset(base + "filigrane.png",      "image/png")
                        .ifPresent(uri -> assets.put("filigraneUri", uri));
                loadAsset(base + "entete-divers.png",  "image/png")
                        .ifPresent(uri -> assets.put("enteteUri", uri));
                yield assets;
            }

            default -> assets;
        };
    }

    private java.util.Optional<String> loadAsset(String resourcePath, String mimeType) {
        try (InputStream is = getClass().getClassLoader().getResourceAsStream(resourcePath)) {
            if (is == null) {
                log.debug("  Asset absent (ignored): {}", resourcePath);
                return java.util.Optional.empty();
            }
            byte[] bytes = is.readAllBytes();
            String dataUri = "data:" + mimeType + ";base64,"
                           + Base64.getEncoder().encodeToString(bytes);
            log.debug("  Loaded: {} ({} KB)", resourcePath, bytes.length / 1024);
            return java.util.Optional.of(dataUri);
        } catch (IOException e) {
            log.warn("  Error loading asset {}: {}", resourcePath, e.getMessage());
            return java.util.Optional.empty();
        }
    }

    private long estimateSizeKb() {
        long total = commonAssets.values().stream().mapToLong(String::length).sum();
        total += templateAssets.values().stream()
                .flatMap(m -> m.values().stream())
                .mapToLong(String::length)
                .sum();
        return total / 1024;
    }
}
