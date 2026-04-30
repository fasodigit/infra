// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import com.maxmind.geoip2.DatabaseReader;
import com.maxmind.geoip2.exception.AddressNotFoundException;
import com.maxmind.geoip2.exception.GeoIp2Exception;
import com.maxmind.geoip2.model.CityResponse;
import jakarta.annotation.PostConstruct;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Service;

import java.io.File;
import java.io.IOException;
import java.net.InetAddress;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Optional;

/**
 * MaxMind GeoLite2-City resolver used by {@link RiskScoringService} to compute
 * the IP-distance signal (cf. SECURITY-HARDENING-PLAN-2026-04-30 §4 Tier 5).
 *
 * <p>The {@code GeoLite2-City.mmdb} file is downloaded out-of-tree by
 * {@code INFRA/scripts/download-geolite2.sh} (license CC BY-SA 4.0, compatible
 * with AGPL). Path is configurable via {@code admin.geoip.database-path}
 * (default {@code /var/lib/auth-ms/GeoLite2-City.mmdb}).
 *
 * <p><b>Fail-open</b>: when the database is missing or unreadable at boot, the
 * service logs a WARN and {@link #resolve(String)} returns {@link Optional#empty()}.
 * Callers (the risk scorer) MUST treat empty as "geo unknown — neutral score"
 * to avoid breaking dev/CI environments without the .mmdb. In production the
 * file is provisioned by the platform (Containerfile volume mount).
 *
 * <p>Cache: simple LRU 10 000 entries (poor man's, synchronized
 * {@link java.util.LinkedHashMap}). At ~5k logins/day, cache lifetime is
 * effectively the JVM uptime; fresh resolutions cost &lt;1 ms (mmdb is mmap'd).
 */
@Service
public class GeoIpResolver {

    private static final Logger log = LoggerFactory.getLogger(GeoIpResolver.class);
    private static final int CACHE_MAX = 10_000;

    @Value("${admin.geoip.database-path:/var/lib/auth-ms/GeoLite2-City.mmdb}")
    private String databasePath;

    private DatabaseReader reader;

    private final Map<String, GeoLocation> cache = Collections.synchronizedMap(
            new LinkedHashMap<>(256, 0.75f, true) {
                @Override
                protected boolean removeEldestEntry(Map.Entry<String, GeoLocation> eldest) {
                    return size() > CACHE_MAX;
                }
            });

    @PostConstruct
    public void init() {
        try {
            File db = new File(databasePath);
            if (!db.exists() || !db.canRead()) {
                log.warn("GeoLite2 database not found at {} — geo signals will be neutral (fail-open). " +
                        "Run INFRA/scripts/download-geolite2.sh to provision it.", databasePath);
                this.reader = null;
                return;
            }
            this.reader = new DatabaseReader.Builder(db).withCache(
                    new com.maxmind.db.CHMCache()).build();
            log.info("GeoLite2 database loaded from {} ({} bytes)", databasePath, db.length());
        } catch (IOException e) {
            log.warn("Failed to load GeoLite2 database from {}: {} — geo signals will be neutral.",
                    databasePath, e.getMessage());
            this.reader = null;
        }
    }

    /**
     * Resolve an IP address to a {@link GeoLocation}. Empty when:
     * <ul>
     *   <li>the {@code .mmdb} is missing (fail-open dev mode);</li>
     *   <li>the IP is private / unrecognised by MaxMind;</li>
     *   <li>parsing fails for any reason.</li>
     * </ul>
     */
    public Optional<GeoLocation> resolve(String ip) {
        if (ip == null || ip.isBlank() || reader == null) return Optional.empty();
        GeoLocation cached = cache.get(ip);
        if (cached != null) return Optional.of(cached);
        try {
            CityResponse resp = reader.city(InetAddress.getByName(ip));
            String country = resp.getCountry() == null ? null : resp.getCountry().getIsoCode();
            String city = resp.getCity() == null ? null : resp.getCity().getName();
            Double lat = resp.getLocation() == null ? null : resp.getLocation().getLatitude();
            Double lon = resp.getLocation() == null ? null : resp.getLocation().getLongitude();
            GeoLocation g = new GeoLocation(country, city, lat, lon);
            cache.put(ip, g);
            return Optional.of(g);
        } catch (AddressNotFoundException e) {
            // Common case for private IPs in dev — log at DEBUG to avoid spam.
            log.debug("GeoIP miss (address not found) ip={}", ip);
            return Optional.empty();
        } catch (IOException | GeoIp2Exception e) {
            log.warn("GeoIP resolution failed for ip={}: {}", ip, e.getMessage());
            return Optional.empty();
        }
    }

    /** Whether the database is loaded and ready to serve queries. */
    public boolean isAvailable() {
        return reader != null;
    }

    /**
     * Immutable geo location record. Any field may be {@code null} when the
     * database does not have the corresponding entry (e.g. country-only IPs).
     */
    public record GeoLocation(String country, String city, Double lat, Double lon) {
        /** Haversine distance in kilometres between {@code this} and {@code other}. */
        public double distanceKmTo(GeoLocation other) {
            if (this.lat == null || this.lon == null
                    || other == null || other.lat == null || other.lon == null) {
                return Double.NaN;
            }
            double earthRadiusKm = 6371.0;
            double dLat = Math.toRadians(other.lat - this.lat);
            double dLon = Math.toRadians(other.lon - this.lon);
            double lat1 = Math.toRadians(this.lat);
            double lat2 = Math.toRadians(other.lat);
            double a = Math.sin(dLat / 2) * Math.sin(dLat / 2)
                    + Math.sin(dLon / 2) * Math.sin(dLon / 2) * Math.cos(lat1) * Math.cos(lat2);
            return 2 * earthRadiusKm * Math.asin(Math.sqrt(a));
        }
    }
}
