package bf.gov.faso.poulets.graphql;

import bf.gov.faso.poulets.model.Eleveur;
import bf.gov.faso.poulets.service.EleveurService;
import com.netflix.graphql.dgs.DgsComponent;
import com.netflix.graphql.dgs.DgsMutation;
import com.netflix.graphql.dgs.DgsQuery;
import com.netflix.graphql.dgs.InputArgument;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.domain.Page;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.UUID;

/**
 * DGS data fetcher for Eleveur queries and mutations.
 */
@DgsComponent
public class EleveurDataFetcher {

    private static final Logger log = LoggerFactory.getLogger(EleveurDataFetcher.class);

    private final EleveurService eleveurService;

    public EleveurDataFetcher(EleveurService eleveurService) {
        this.eleveurService = eleveurService;
    }

    @DgsQuery
    public Map<String, Object> eleveurs(@InputArgument String location,
                                        @InputArgument Integer page,
                                        @InputArgument Integer size) {
        int pageNum = (page != null) ? page : 0;
        int pageSize = (size != null) ? size : 20;

        Page<Eleveur> eleveurPage = eleveurService.findAll(location, pageNum, pageSize);

        Map<String, Object> result = new LinkedHashMap<>();
        result.put("content", eleveurPage.getContent());
        result.put("totalElements", (int) eleveurPage.getTotalElements());
        result.put("totalPages", eleveurPage.getTotalPages());
        result.put("page", pageNum);
        result.put("size", pageSize);
        return result;
    }

    @DgsQuery
    public Eleveur eleveur(@InputArgument String id) {
        return eleveurService.findById(UUID.fromString(id)).orElse(null);
    }

    @DgsMutation
    public Eleveur registerEleveur(@InputArgument Map<String, Object> input) {
        String userId = (String) input.get("userId");
        String name = (String) input.get("name");
        String phone = (String) input.get("phone");
        String location = (String) input.get("location");
        String description = (String) input.get("description");

        if (userId == null || userId.isBlank()) {
            userId = UUID.randomUUID().toString();
        }

        return eleveurService.register(userId, name, phone, location, description);
    }

    @DgsMutation
    public Eleveur updateEleveur(@InputArgument String id, @InputArgument Map<String, Object> input) {
        String name = (String) input.get("name");
        String phone = (String) input.get("phone");
        String location = (String) input.get("location");
        String description = (String) input.get("description");

        return eleveurService.update(UUID.fromString(id), name, phone, location, description);
    }
}
