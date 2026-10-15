# Compose image digest proposal

`helixtest/docker/docker-compose.yml` was not edited. Each image was inspected
with `docker buildx imagetools inspect` on 2026-10-14. None resolved.

| Compose image | Current tag | Result |
|---|---|---|
| `ghcr.io/example/mock-wes:latest` | `latest` | not found |
| `ghcr.io/example/mock-tes:latest` | `latest` | not found |
| `ghcr.io/example/mock-drs:latest` | `latest` | not found |
| `ghcr.io/example/mock-trs:latest` | `latest` | not found |
| `ghcr.io/example/mock-beacon:latest` | `latest` | not found |
| `ghcr.io/example/mock-oidc:latest` | `latest` | not found |

These names are placeholders. No digest is proposed because the registry
returned no manifest.
