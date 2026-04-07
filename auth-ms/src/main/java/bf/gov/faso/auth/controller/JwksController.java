package bf.gov.faso.auth.controller;

import bf.gov.faso.auth.service.JwtService;
import org.springframework.http.CacheControl;
import org.springframework.http.MediaType;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RestController;

import java.util.Map;
import java.util.concurrent.TimeUnit;

/**
 * JWKS endpoint: /.well-known/jwks.json
 * <p>
 * This endpoint is consumed by ARMAGEDDON's jwt_authn filter to fetch
 * the public keys used for JWT signature verification.
 * <p>
 * Public, no authentication required.
 * Cached for 5 minutes (Cache-Control header) to reduce load while
 * ensuring key rotations propagate within a reasonable window.
 */
@RestController
public class JwksController {

    private final JwtService jwtService;

    public JwksController(JwtService jwtService) {
        this.jwtService = jwtService;
    }

    @GetMapping(value = "/.well-known/jwks.json", produces = MediaType.APPLICATION_JSON_VALUE)
    public ResponseEntity<Map<String, Object>> jwks() {
        Map<String, Object> jwks = jwtService.buildJwks();

        return ResponseEntity.ok()
                .cacheControl(CacheControl.maxAge(5, TimeUnit.MINUTES).mustRevalidate())
                .body(jwks);
    }
}
