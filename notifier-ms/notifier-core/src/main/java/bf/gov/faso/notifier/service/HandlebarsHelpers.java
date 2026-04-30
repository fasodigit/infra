/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.service;

import com.github.jknack.handlebars.Options;

import java.io.IOException;
import java.util.Objects;

/**
 * HandlebarsHelpers — custom Handlebars helpers registered at startup.
 */
public class HandlebarsHelpers {

    /** Truncate a string to maxLen characters, appending "…" if truncated. */
    public static CharSequence truncate(String value, Options options) throws IOException {
        if (value == null) return "";
        int maxLen = options.hash("len", 80);
        if (value.length() <= maxLen) return value;
        return value.substring(0, maxLen) + "…";
    }

    /** Return "FASO" branding constant for templates. */
    public static String faso() {
        return "FASO DIGITALISATION";
    }

    /** Shorten a SHA to 7 chars. */
    public static CharSequence shortSha(String sha) {
        if (sha == null || sha.length() < 7) return sha != null ? sha : "";
        return sha.substring(0, 7);
    }

    /**
     * Block helper {{#eq a b}}...{{else}}...{{/eq}} — renders the inverse block when
     * the two operands differ (Object#equals). Used for language switching in admin templates.
     */
    public static CharSequence eq(Object a, Object b, Options options) throws IOException {
        return Objects.equals(a, b) ? options.fn() : options.inverse();
    }
}
