package bf.gov.faso.poulets.graphql;

import bf.gov.faso.poulets.model.Client;
import bf.gov.faso.poulets.service.ClientService;
import com.netflix.graphql.dgs.DgsComponent;
import com.netflix.graphql.dgs.DgsMutation;
import com.netflix.graphql.dgs.InputArgument;

import java.util.Map;
import java.util.UUID;

/**
 * DGS data fetcher for Client mutations.
 */
@DgsComponent
public class ClientDataFetcher {

    private final ClientService clientService;

    public ClientDataFetcher(ClientService clientService) {
        this.clientService = clientService;
    }

    @DgsMutation
    public Client registerClient(@InputArgument Map<String, Object> input) {
        String userId = (String) input.get("userId");
        String name = (String) input.get("name");
        String phone = (String) input.get("phone");
        String address = (String) input.get("address");

        if (userId == null || userId.isBlank()) {
            userId = UUID.randomUUID().toString();
        }

        return clientService.register(userId, name, phone, address);
    }
}
