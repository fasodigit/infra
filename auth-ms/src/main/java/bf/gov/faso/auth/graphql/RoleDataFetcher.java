package bf.gov.faso.auth.graphql;

import bf.gov.faso.auth.model.Role;
import bf.gov.faso.auth.repository.RoleRepository;
import com.netflix.graphql.dgs.DgsComponent;
import com.netflix.graphql.dgs.DgsQuery;
import org.springframework.security.access.prepost.PreAuthorize;

import java.util.List;

/**
 * DGS data fetcher for Role queries.
 */
@DgsComponent
public class RoleDataFetcher {

    private final RoleRepository roleRepository;

    public RoleDataFetcher(RoleRepository roleRepository) {
        this.roleRepository = roleRepository;
    }

    @DgsQuery
    @PreAuthorize("hasAnyRole('SUPER_ADMIN', 'ADMIN', 'OPERATOR')")
    public List<Role> roles() {
        return roleRepository.findAll();
    }
}
