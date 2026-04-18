package bf.gov.faso.renderer.health;

import bf.gov.faso.renderer.service.AssetInliner;
import bf.gov.faso.renderer.service.PlaywrightMultiBrowserPool;
import bf.gov.faso.renderer.service.TemplateService;
import org.springframework.boot.actuate.health.Health;
import org.springframework.boot.actuate.health.ReactiveHealthIndicator;
import org.springframework.stereotype.Component;
import reactor.core.publisher.Mono;

@Component("readinessState")
public class RendererReadinessIndicator implements ReactiveHealthIndicator {

    private static final double READINESS_THRESHOLD = 0.80;

    private final PlaywrightMultiBrowserPool pool;
    private final TemplateService            templateService;
    private final AssetInliner               assetInliner;

    public RendererReadinessIndicator(
            PlaywrightMultiBrowserPool pool,
            TemplateService templateService,
            AssetInliner assetInliner) {
        this.pool            = pool;
        this.templateService = templateService;
        this.assetInliner    = assetInliner;
    }

    @Override
    public Mono<Health> health() {
        return Mono.fromCallable(this::checkReadiness);
    }

    private Health checkReadiness() {
        int available = pool.available();
        int total     = pool.total();
        int templates = templateService.availableTemplates().size();
        int assets    = assetInliner.totalAssetCount();

        double availability = total > 0 ? (double) available / total : 0.0;
        boolean poolHealthy = pool.isHealthy();
        boolean poolReady   = availability >= READINESS_THRESHOLD;
        boolean hasTemplates = templates > 0;

        Health.Builder builder = (poolHealthy && poolReady && hasTemplates)
                ? Health.up() : Health.down();

        builder
            .withDetail("pool.available",    available)
            .withDetail("pool.total",        total)
            .withDetail("pool.availability", String.format("%.0f%%", availability * 100))
            .withDetail("pool.threshold",    String.format("%.0f%%", READINESS_THRESHOLD * 100))
            .withDetail("pool.healthy",      poolHealthy)
            .withDetail("templates.count",   templates)
            .withDetail("assets.count",      assets)
            .withDetail("browsers.count",    pool.getBrowserCount());

        if (!poolReady) {
            builder.withDetail("reason",
                    "Pool warming up: %.0f%% < %.0f%% threshold"
                    .formatted(availability * 100, READINESS_THRESHOLD * 100));
        }
        if (!hasTemplates) {
            builder.withDetail("reason", "No Handlebars templates loaded");
        }

        return builder.build();
    }
}
