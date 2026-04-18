package bf.gov.faso.cache;

import bf.gov.faso.cache.bloom.BloomFilterService;
import bf.gov.faso.cache.dragonfly.DragonflyDBCacheService;
import bf.gov.faso.cache.hazelcast.HazelcastCacheService;
import bf.gov.faso.cache.lookup.ThreeTierLookupService;
import bf.gov.faso.cache.stream.StreamConsumerProperties;
import bf.gov.faso.cache.stream.StreamOutboxService;
import bf.gov.faso.cache.stream.WorkflowStreamService;
import bf.gov.faso.cache.warmup.CacheWarmUpService;
import bf.gov.faso.cache.warmup.WarmUpDataProvider;
import bf.gov.faso.cache.warmup.WarmUpProperties;
import com.fasterxml.jackson.databind.ObjectMapper;
import io.lettuce.core.ClientOptions;
import io.lettuce.core.protocol.ProtocolVersion;
import org.springframework.boot.autoconfigure.AutoConfiguration;
import org.springframework.boot.autoconfigure.condition.ConditionalOnClass;
import org.springframework.boot.autoconfigure.condition.ConditionalOnMissingBean;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.boot.autoconfigure.data.redis.LettuceClientConfigurationBuilderCustomizer;
import org.springframework.boot.context.properties.EnableConfigurationProperties;
import org.springframework.context.annotation.Bean;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.data.redis.serializer.StringRedisSerializer;
import org.springframework.data.redis.connection.RedisConnectionFactory;

import java.util.List;

/**
 * Auto-configuration for ETAT-CIVIL cache infrastructure.
 * <p>
 * Registers beans conditionally:
 * <ul>
 *     <li>{@link BloomFilterService} — when StringRedisTemplate is available</li>
 *     <li>{@link DragonflyDBCacheService} — when StringRedisTemplate is available</li>
 *     <li>{@link HazelcastCacheService} — when Hazelcast is on the classpath</li>
 *     <li>{@link StreamOutboxService} — when {@code ec.cache.stream.enabled=true}</li>
 *     <li>{@link CacheWarmUpService} — when {@code ec.cache.warmup.enabled=true}</li>
 *     <li>{@link ThreeTierLookupService} — always (depends on Bloom + DragonflyDB)</li>
 * </ul>
 * <p>
 * {@code @ConditionalOnMissingBean} ensures services with local overrides take precedence.
 */
@AutoConfiguration
@EnableConfigurationProperties({CacheProperties.class, StreamConsumerProperties.class, WarmUpProperties.class})
@ConditionalOnClass(StringRedisTemplate.class)
public class CacheAutoConfiguration {

    /**
     * Force RESP2 protocol for DragonflyDB compatibility.
     * DragonflyDB v1.15 doesn't support RESP3 HELLO+AUTH handshake used by Lettuce 6.3+.
     * This customizer applies to Spring Boot's auto-configured LettuceConnectionFactory.
     */
    @Bean
    @ConditionalOnMissingBean
    public LettuceClientConfigurationBuilderCustomizer lettuceResp2Customizer() {
        return builder -> builder.clientOptions(
                ClientOptions.builder()
                        .protocolVersion(ProtocolVersion.RESP2)
                        .build());
    }

    @Bean
    @ConditionalOnMissingBean(name = "stringRedisTemplate")
    public StringRedisTemplate stringRedisTemplate(RedisConnectionFactory connectionFactory) {
        var template = new StringRedisTemplate();
        template.setConnectionFactory(connectionFactory);
        template.setKeySerializer(new StringRedisSerializer());
        template.setValueSerializer(new StringRedisSerializer());
        template.setHashKeySerializer(new StringRedisSerializer());
        template.setHashValueSerializer(new StringRedisSerializer());
        template.afterPropertiesSet();
        return template;
    }

    @Bean
    @ConditionalOnMissingBean
    public BloomFilterService bloomFilterService(StringRedisTemplate redisTemplate,
                                                  CacheProperties properties) {
        return new BloomFilterService(redisTemplate, properties);
    }

    @Bean
    @ConditionalOnMissingBean
    public DragonflyDBCacheService dragonflyDBCacheService(StringRedisTemplate redisTemplate,
                                                            ObjectMapper objectMapper,
                                                            CacheProperties properties) {
        return new DragonflyDBCacheService(redisTemplate, objectMapper, properties);
    }

    @Bean
    @ConditionalOnMissingBean
    @ConditionalOnClass(name = "com.hazelcast.core.HazelcastInstance")
    public HazelcastCacheService hazelcastCacheService() {
        return new HazelcastCacheService();
    }

    // ── Stream infrastructure ─────────────────────────────────────────

    @Bean
    @ConditionalOnMissingBean
    @ConditionalOnProperty(name = "ec.cache.stream.enabled", havingValue = "true")
    public StreamOutboxService streamOutboxService(StringRedisTemplate redisTemplate,
                                                    StreamConsumerProperties streamProperties) {
        return new StreamOutboxService(redisTemplate, streamProperties);
    }

    @Bean
    @ConditionalOnMissingBean
    @ConditionalOnProperty(name = "ec.cache.stream.enabled", havingValue = "true")
    public WorkflowStreamService workflowStreamService(StreamOutboxService streamOutboxService,
                                                        StreamConsumerProperties streamProperties,
                                                        StringRedisTemplate redisTemplate) {
        return new WorkflowStreamService(streamOutboxService, streamProperties, redisTemplate);
    }

    // ── Warm-up infrastructure ────────────────────────────────────────

    @Bean
    @ConditionalOnMissingBean
    @ConditionalOnProperty(name = "ec.cache.warmup.enabled", havingValue = "true")
    public CacheWarmUpService cacheWarmUpService(DragonflyDBCacheService cacheService,
                                                  BloomFilterService bloomFilterService,
                                                  WarmUpProperties warmUpProperties,
                                                  List<WarmUpDataProvider<?>> dataProviders) {
        return new CacheWarmUpService(cacheService, bloomFilterService, warmUpProperties, dataProviders);
    }

    // ── Three-tier lookup ─────────────────────────────────────────────

    @Bean
    @ConditionalOnMissingBean
    public ThreeTierLookupService threeTierLookupService(BloomFilterService bloomFilterService,
                                                          DragonflyDBCacheService cacheService) {
        return new ThreeTierLookupService(bloomFilterService, cacheService);
    }
}
