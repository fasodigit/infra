package bf.gov.faso.poulets.graphql;

import bf.gov.faso.poulets.model.Categorie;
import bf.gov.faso.poulets.repository.CategorieRepository;
import com.netflix.graphql.dgs.DgsComponent;
import com.netflix.graphql.dgs.DgsQuery;

import java.util.List;

/**
 * DGS data fetcher for Categorie queries.
 */
@DgsComponent
public class CategorieDataFetcher {

    private final CategorieRepository categorieRepository;

    public CategorieDataFetcher(CategorieRepository categorieRepository) {
        this.categorieRepository = categorieRepository;
    }

    @DgsQuery
    public List<Categorie> categories() {
        return categorieRepository.findAll();
    }
}
