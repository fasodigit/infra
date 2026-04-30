// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.config;

import bf.gov.faso.auth.controller.admin.PushApprovalWsController;
import org.springframework.context.annotation.Configuration;
import org.springframework.web.socket.config.annotation.EnableWebSocket;
import org.springframework.web.socket.config.annotation.WebSocketConfigurer;
import org.springframework.web.socket.config.annotation.WebSocketHandlerRegistry;

/**
 * Spring WebSocket configuration — registers the push-approval handler.
 *
 * <h2>Endpoint</h2>
 * {@code /internal/ws/approval} — internal path proxied by ARMAGEDDON on
 * {@code /ws/admin/approval}.  ARMAGEDDON validates the JWT before the
 * upgrade; auth-ms trusts the {@code X-User-Id} header injected by the proxy.
 *
 * <h2>CORS / allowed origins</h2>
 * Since ARMAGEDDON is the only caller of this internal endpoint, the allowed
 * origin is restricted to the ARMAGEDDON service address.  In dev the wildcard
 * {@code *} is used; production deployments override via
 * {@code admin.push-approval.allowed-origins}.
 *
 * <h2>SockJS</h2>
 * SockJS is intentionally NOT enabled — ARMAGEDDON performs a native WebSocket
 * upgrade; SockJS fallback transport is unnecessary and adds complexity.
 */
@Configuration
@EnableWebSocket
public class WebSocketConfig implements WebSocketConfigurer {

    private final PushApprovalWsController pushApprovalWsController;

    public WebSocketConfig(PushApprovalWsController pushApprovalWsController) {
        this.pushApprovalWsController = pushApprovalWsController;
    }

    @Override
    public void registerWebSocketHandlers(WebSocketHandlerRegistry registry) {
        registry.addHandler(pushApprovalWsController, "/internal/ws/approval")
                // Allow ARMAGEDDON origin + localhost for dev.
                // Production: set admin.push-approval.allowed-origins in Vault/Consul.
                .setAllowedOriginPatterns("http://armageddon:*", "http://localhost:*", "https://*.faso.bf");
    }
}
