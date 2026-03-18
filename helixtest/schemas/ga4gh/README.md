# GA4GH official schemas

Vendored OpenAPI definitions from GA4GH for use in HelixTest validation. Loading and validation are implemented in `common::ga4gh_schemas`.

| File | Source | Used for |
|------|--------|----------|
| `wes-openapi.yaml` | [workflow-execution-service-schemas](https://ga4gh.github.io/workflow-execution-service-schemas/openapi.yaml) (WES 1.1.0) | GET /service-info → `ServiceInfo` |
| `tes-openapi.yaml` | [task-execution-schemas](https://github.com/ga4gh/task-execution-schemas) (TES 1.1.0) | POST /tasks → `tesCreateTaskResponse`; GET /tasks/{id} → `tesTask` |
| `trs-openapi.yaml` | [tool-registry-service-schemas](https://github.com/ga4gh/tool-registry-service-schemas) develop (TRS 2.1.0) | GET /tools, /tools/{id} → `Tool`; GET /tools/{id}/versions → `ToolVersion` |

To update a schema when GA4GH releases a new version, replace the corresponding file and re-run tests (`cargo test -p common ga4gh_schemas`).
