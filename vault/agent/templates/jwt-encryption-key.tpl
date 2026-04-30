{{- /* SPDX-License-Identifier: AGPL-3.0-or-later */ -}}
{{- /* JWT signing-key encryption key (auth-ms only). */ -}}
{{ with secret "faso/data/auth-ms/jwt" }}
auth.jwt.encryption-key-b64={{ .Data.data.encryption_key_b64 }}
{{ end -}}
