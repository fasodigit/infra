// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.crypto;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Algorithm + parameters used to produce a stored hash. Persisted as JSONB in
 * {@code users.hash_params} so verifications remain deterministic across
 * default-parameter migrations.
 *
 * @param algo            "argon2id" | "bcrypt" | "sha256" (legacy)
 * @param memory          Argon2 memory cost (KiB). Ignored for bcrypt.
 * @param iterations      Argon2 time cost. Ignored for bcrypt.
 * @param parallelism     Argon2 parallelism. Ignored for bcrypt.
 * @param argonVersion    Argon2 version (19 = 1.3, the only spec value today).
 * @param pepperVersion   HMAC pepper rotation index (0 = no pepper).
 */
public record HashParams(
        String algo,
        int memory,
        int iterations,
        int parallelism,
        int argonVersion,
        int pepperVersion
) {

    public static HashParams argon2id(int m, int t, int p, int pepperVersion) {
        return new HashParams("argon2id", m, t, p, 19, pepperVersion);
    }

    public static HashParams bcryptLegacy() {
        return new HashParams("bcrypt", 0, 0, 0, 0, 0);
    }

    /** True when the stored hash should be re-hashed on next login. */
    public boolean needsRehash(HashParams current) {
        if (!"argon2id".equals(this.algo)) return true;
        if (this.pepperVersion < current.pepperVersion) return true;
        return this.memory < current.memory
                || this.iterations < current.iterations
                || this.parallelism < current.parallelism;
    }

    public Map<String, Object> toMap() {
        Map<String, Object> m = new LinkedHashMap<>();
        m.put("algo", algo);
        m.put("m", memory);
        m.put("t", iterations);
        m.put("p", parallelism);
        m.put("version", argonVersion);
        m.put("pepperVersion", pepperVersion);
        return m;
    }
}
