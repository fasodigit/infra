//! Header masking: remove headers that leak server information.

/// Default headers to strip from responses (server fingerprinting prevention).
pub const DEFAULT_MASKED_HEADERS: &[&str] = &[
    "server",
    "x-powered-by",
    "x-aspnet-version",
    "x-aspnetmvc-version",
    "x-runtime",
    "x-version",
    "x-generator",
    "x-drupal-cache",
    "x-varnish",
    "via",
    "x-cache",
    "x-cache-hits",
    "x-served-by",
    "x-timer",
    "x-request-id",  // internal request IDs should not leak
];

/// Check if a header name should be masked.
pub fn should_mask(header_name: &str) -> bool {
    DEFAULT_MASKED_HEADERS
        .iter()
        .any(|h| h.eq_ignore_ascii_case(header_name))
}
