# GA4GH official schemas

Vendored **official** OpenAPI definitions from GA4GH. These files — not Ferrum’s utoipa dump — are the schema source of truth for HelixTest validation. Loading: `common::ga4gh_schemas`.

| File | Source | Used for |
|------|--------|----------|
| `wes-openapi.yaml` | [workflow-execution-service-schemas](https://ga4gh.github.io/workflow-execution-service-schemas/openapi.yaml) (WES 1.1.0) | GET /service-info → `ServiceInfo` |
| `tes-openapi.yaml` | [task-execution-schemas](https://github.com/ga4gh/task-execution-schemas) (TES 1.1.0) | POST /tasks → `tesCreateTaskResponse`; GET /tasks/{id} → `tesTask` |
| `trs-openapi.yaml` | [tool-registry-service-schemas](https://github.com/ga4gh/tool-registry-service-schemas) develop (TRS 2.1.0) | GET /tools, /tools/{id} → `Tool`; GET /tools/{id}/versions → `ToolVersion` |
| `htsget-openapi.yaml` | [samtools/hts-specs](https://github.com/samtools/hts-specs) `pub/htsget-openapi.yaml` (htsget **1.3.0**); bundled `Ga4ghService`/`Ga4ghServiceType` replace external service-info `$ref` for offline deref | `htsgetServiceInfo`, `htsgetResponseReads`, `htsgetResponseVariants`, `Error` |
| `drs-openapi.yaml` | [DRS 1.4.0 OpenAPI](https://ga4gh.github.io/data-repository-service-schemas/preview/release/drs-1.4.0/openapi.yaml) | GET `/objects/{id}` → `DrsObject` |
| `beacon-boolean-response.json` | [Beacon v2 `beaconBooleanResponse`](https://github.com/ga4gh-beacon/beacon-v2/blob/main/framework/json/responses/beaconBooleanResponse.json) inlined to draft-07 | POST `/query` boolean body → `meta` + `responseSummary.exists` |

To update a schema when GA4GH releases a new version, replace the corresponding file and re-run tests (`cargo test -p common ga4gh_schemas`).
