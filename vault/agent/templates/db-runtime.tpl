{{- /* SPDX-License-Identifier: AGPL-3.0-or-later */ -}}
{{- /* Runtime DB credentials for HikariCP (DML only, lease 1h, auto-renew). */ -}}
{{ with secret (printf "database/creds/%s-runtime-role" (env "FASO_SERVICE_NAME")) }}
spring.datasource.username={{ .Data.username }}
spring.datasource.password={{ .Data.password }}
{{ end -}}
