package armageddon.authz

# Default deny
default allow := false

# Allow if user is authenticated (has a subject claim)
allow if {
    input.auth.claims.sub
}

# Allow health check endpoints without authentication
allow if {
    startswith(input.request.path, "/health/")
}

# Allow Kratos self-service endpoints without authentication
allow if {
    startswith(input.request.path, "/self-service/")
}
