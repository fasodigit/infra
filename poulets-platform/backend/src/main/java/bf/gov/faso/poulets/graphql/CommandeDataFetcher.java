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
    public Map<String, Object> mesCommandes(@InputArgument String status,
                                            @InputArgument Integer page,
                                            @InputArgument Integer size) {
        int pageNum = (page != null) ? page : 0;
        int pageSize = (size != null) ? size : 20;

        // In a real scenario, we would extract the user ID from the security context
        // and find the client/eleveur by userId. For now, this returns an empty page
        // unless a clientId or eleveurId is passed through context.
        // This is a placeholder that will work once ARMAGEDDON passes user info.

        CommandeStatus commandeStatus = (status != null) ? CommandeStatus.valueOf(status) : null;

        // Return empty page as placeholder -- in production, extract userId from
        // ARMAGEDDON-forwarded headers and look up the client/eleveur
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
    @SuppressWarnings("unchecked")
    public Commande createCommande(@InputArgument Map<String, Object> input) {
        UUID clientId = UUID.fromString((String) input.get("clientId"));
        UUID eleveurId = UUID.fromString((String) input.get("eleveurId"));
        List<Map<String, Object>> items = (List<Map<String, Object>>) input.get("items");

        return commandeService.create(clientId, eleveurId, items);
    }

    @DgsMutation
    public Commande cancelCommande(@InputArgument String id) {
        return commandeService.cancel(UUID.fromString(id));
    }

    @DgsMutation
    public Commande confirmCommande(@InputArgument String id) {
        return commandeService.confirm(UUID.fromString(id));
    }

    @DgsMutation
    public Commande markReady(@InputArgument String id) {
        return commandeService.markReady(UUID.fromString(id));
    }

    @DgsMutation
    public Commande markDelivered(@InputArgument String id) {
        return commandeService.markDelivered(UUID.fromString(id));
    }
}
