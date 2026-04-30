// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.repository;

import bf.gov.faso.auth.model.RecoveryCode;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.UUID;

@Repository
public interface RecoveryCodeRepository extends JpaRepository<RecoveryCode, UUID> {

    @Query("SELECT rc FROM RecoveryCode rc WHERE rc.userId = :userId AND rc.usedAt IS NULL")
    List<RecoveryCode> findUnusedByUserId(@Param("userId") UUID userId);

    @Query("SELECT COUNT(rc) FROM RecoveryCode rc WHERE rc.userId = :userId AND rc.usedAt IS NULL")
    long countUnusedByUserId(@Param("userId") UUID userId);

    List<RecoveryCode> findByUserIdOrderByGeneratedAtDesc(UUID userId);
}
