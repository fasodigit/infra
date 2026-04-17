package bf.gov.faso.poulets.graphql;

import bf.gov.faso.poulets.model.Poulet;
import bf.gov.faso.poulets.model.Race;
import bf.gov.faso.poulets.service.PouletService;
import com.netflix.graphql.dgs.DgsComponent;
import com.netflix.graphql.dgs.DgsMutation;
import com.netflix.graphql.dgs.DgsQuery;
import com.netflix.graphql.dgs.InputArgument;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.domain.Page;
import org.springframework.security.access.prepost.PreAuthorize;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.UUID;

/**
 * DGS data fetcher for Poulet queries and mutations.
 */
@DgsComponent
public class PouletDataFetcher {

    private static final Logger log = LoggerFactory.getLogger(PouletDataFetcher.class);

    private final PouletService pouletService;

    public PouletDataFetcher(PouletService pouletService) {
        this.pouletService = pouletService;
    }

    @DgsQuery
    public Map<String, Object> poulets(@InputArgument Map<String, Object> filter,
                                       @InputArgument Integer page,
                                       @InputArgument Integer size) {
        int pageNum = (page != null) ? page : 0;
        int pageSize = (size != null) ? size : 20;

        Page<Poulet> pouletPage = pouletService.findAll(filter, pageNum, pageSize);

        Map<String, Object> result = new LinkedHashMap<>();
        result.put("content", pouletPage.getContent());
        result.put("totalElements", (int) pouletPage.getTotalElements());
        result.put("totalPages", pouletPage.getTotalPages());
        result.put("page", pageNum);
        result.put("size", pageSize);
        return result;
    }

    @DgsQuery
    public Poulet poulet(@InputArgument String id) {
        return pouletService.findById(UUID.fromString(id)).orElse(null);
    }

    @DgsMutation
    @PreAuthorize("hasRole('ELEVEUR') or hasRole('ADMIN')")
    public Poulet addPoulet(@InputArgument Map<String, Object> input) {
        UUID eleveurId = UUID.fromString((String) input.get("eleveurId"));
        Race race = Race.valueOf((String) input.get("race"));
        double weight = ((Number) input.get("weight")).doubleValue();
        double price = ((Number) input.get("price")).doubleValue();
        int quantity = ((Number) input.get("quantity")).intValue();
        String description = (String) input.get("description");
        UUID categorieId = input.containsKey("categorieId") && input.get("categorieId") != null
                ? UUID.fromString((String) input.get("categorieId")) : null;

        return pouletService.add(eleveurId, race, weight, price, quantity, description, categorieId);
    }

    @DgsMutation
    @PreAuthorize("hasRole('ELEVEUR') or hasRole('ADMIN')")
    public Poulet updatePoulet(@InputArgument String id, @InputArgument Map<String, Object> input) {
        Race race = input.containsKey("race") ? Race.valueOf((String) input.get("race")) : null;
        Double weight = input.containsKey("weight") ? ((Number) input.get("weight")).doubleValue() : null;
        Double price = input.containsKey("price") ? ((Number) input.get("price")).doubleValue() : null;
        Integer quantity = input.containsKey("quantity") ? ((Number) input.get("quantity")).intValue() : null;
        String description = (String) input.get("description");
        UUID categorieId = input.containsKey("categorieId") && input.get("categorieId") != null
                ? UUID.fromString((String) input.get("categorieId")) : null;

        return pouletService.update(UUID.fromString(id), race, weight, price, quantity, description, categorieId);
    }

    @DgsMutation
    @PreAuthorize("hasRole('ELEVEUR') or hasRole('ADMIN')")
    public boolean deletePoulet(@InputArgument String id) {
        return pouletService.delete(UUID.fromString(id));
    }
}
