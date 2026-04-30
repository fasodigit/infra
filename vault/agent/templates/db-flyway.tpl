{{- /* SPDX-License-Identifier: AGPL-3.0-or-later */ -}}
{{- /* Flyway DDL credentials, one-shot at boot, lease 30m. */ -}}
{{ with secret (printf "database/creds/%s-flyway-role" (env "FASO_SERVICE_NAME")) }}
spring.flyway.user={{ .Data.username }}
spring.flyway.password={{ .Data.password }}
{{ end -}}
