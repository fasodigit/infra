package bf.gov.faso.poulets.graphql;

import bf.gov.faso.poulets.model.Commande;
import bf.gov.faso.poulets.model.CommandeStatus;
import bf.gov.faso.poulets.service.ClientService;
import bf.gov.faso.poulets.service.CommandeService;
import com.netflix.graphql.dgs.DgsComponent;
import com.netflix.graphql.dgs.DgsMutation;
import com.netflix.graphql.dgs.DgsQuery;
import com.netflix.graphql.dgs.InputArgument;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.domain.Page;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.security.core.Authentication;
import org.springframework.security.core.context.SecurityContextHolder;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * DGS data fetcher for Commande queries and mutations.
 */
@DgsComponent
public class CommandeDataFetcher {

    private static final Logger log = LoggerFactory.getLogger(CommandeDataFetcher.class);

    private final CommandeService commandeService;
    private final ClientService clientService;

    public CommandeDataFetcher(CommandeService commandeService, ClientService clientService) {
        this.commandeService = commandeService;
        this.clientService = clientService;
    }

    @DgsQuery
    @PreAuthorize("isAuthenticated()")
    public Map<String, Object> mesCommandes(@InputArgument String status,
                                            @InputArgument Integer page,
                                            @InputArgument Integer size) {
        int pageNum = (page != null) ? page : 0;
        int pageSize = (size != null) ? size : 20;

        CommandeStatus commandeStatus = (status != null) ? CommandeStatus.valueOf(status) : null;

        // Extract authenticated userId from SecurityContext
        Authentication authentication = SecurityContextHolder.getContext().getAuthentication();
        String userId = (authentication != null) ? (String) authentication.getPrincipal() : null;
        log.debug("mesCommandes called by userId={}", userId);

        Map<String, Object> result = new LinkedHashMap<>();
        result.put("content", List.of());
        result.put("totalElements", 0);
        result.put("totalPages", 0);
        result.put("page", pageNum);
        result.put("size", pageSize);
        return result;
    }

    @DgsQuery
    public Commande commande(@InputArgument String id) {
        return commandeService.findById(UUID.fromString(id)).orElse(null);
    }

    @DgsMutation
    @PreAuthorize("hasRole('USER') or hasRole('CLIENT') or hasRole('ADMIN')")
    @SuppressWarnings("unchecked")
    public Commande createCommande(@InputArgument Map<String, Object> input) {
        UUID clientId = UUID.fromString((String) input.get("clientId"));
        UUID eleveurId = UUID.fromString((String) input.get("eleveurId"));
        List<Map<String, Object>> items = (List<Map<String, Object>>) input.get("items");

        return commandeService.create(clientId, eleveurId, items);
    }

    @DgsMutation
    @PreAuthorize("hasRole('USER') or hasRole('CLIENT') or hasRole('ADMIN')")
    public Commande cancelCommande(@InputArgument String id) {
        return commandeService.cancel(UUID.fromString(id));
    }

    @DgsMutation
    @PreAuthorize("hasRole('ELEVEUR') or hasRole('ADMIN')")
    public Commande confirmCommande(@InputArgument String id) {
        return commandeService.confirm(UUID.fromString(id));
    }

    @DgsMutation
    @PreAuthorize("hasRole('ELEVEUR') or hasRole('ADMIN')")
    public Commande markReady(@InputArgument String id) {
        return commandeService.markReady(UUID.fromString(id));
    }

    @DgsMutation
    @PreAuthorize("hasRole('ELEVEUR') or hasRole('ADMIN')")
    public Commande markDelivered(@InputArgument String id) {
        return commandeService.markDelivered(UUID.fromString(id));
    }
}
