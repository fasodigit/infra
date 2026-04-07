package bf.gov.faso.auth.graphql;

import bf.gov.faso.auth.model.User;
import bf.gov.faso.auth.repository.UserRepository;
import bf.gov.faso.auth.security.JwtAuthenticatedPrincipal;
import com.netflix.graphql.dgs.DgsComponent;
import com.netflix.graphql.dgs.DgsQuery;
import com.netflix.graphql.dgs.InputArgument;
import graphql.schema.DataFetchingEnvironment;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Sort;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.security.core.context.SecurityContextHolder;

import java.util.*;

/**
 * DGS data fetcher for User queries.
 * <p>
 * Provides:
 * - users(page, size): paginated list of all users
 * - user(id): single user by ID
 * - me: current authenticated user
 */
@DgsComponent
public class UserDataFetcher {

    private static final Logger log = LoggerFactory.getLogger(UserDataFetcher.class);

    private final UserRepository userRepository;

    public UserDataFetcher(UserRepository userRepository) {
        this.userRepository = userRepository;
    }

    @DgsQuery
    @PreAuthorize("hasAnyRole('SUPER_ADMIN', 'ADMIN')")
    public Map<String, Object> users(@InputArgument Integer page, @InputArgument Integer size) {
        int pageNum = (page != null) ? page : 0;
        int pageSize = (size != null) ? size : 20;

        Page<User> userPage = userRepository.findAll(
                PageRequest.of(pageNum, pageSize, Sort.by(Sort.Direction.DESC, "createdAt")));

        Map<String, Object> result = new LinkedHashMap<>();
        result.put("content", userPage.getContent());
        result.put("totalElements", (int) userPage.getTotalElements());
        result.put("totalPages", userPage.getTotalPages());
        result.put("page", pageNum);
        result.put("size", pageSize);
        return result;
    }

    @DgsQuery
    @PreAuthorize("hasAnyRole('SUPER_ADMIN', 'ADMIN', 'OPERATOR')")
    public User user(@InputArgument String id) {
        return userRepository.findById(UUID.fromString(id)).orElse(null);
    }

    @DgsQuery
    public User me(DataFetchingEnvironment env) {
        var auth = SecurityContextHolder.getContext().getAuthentication();
        if (auth == null || !(auth.getPrincipal() instanceof JwtAuthenticatedPrincipal principal)) {
            return null;
        }

        String userId = principal.getUserId();
        return userRepository.findById(UUID.fromString(userId)).orElse(null);
    }
}
