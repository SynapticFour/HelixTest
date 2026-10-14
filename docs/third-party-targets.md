# Third-party targets (2026-10)

This page records what the 2026-10-13 runs against Sapporo, Funnel, the GA4GH Starter Kit DRS, and beacon2-pi-api showed. It does not add a profile, a fixture, or a code path for those servers.

Source packet: `evidence/2026-10/e-conformance-compare/` in the business repository (`REVIEW-PACKET.md`, `MATRIX.md`, `HELIXTEST-BUGS.md`, `LIMITS.md`, `results/`). These runs are not GA4GH certification and not a ranking. Where the server's declared version is not the suite pack, the matrix does not score a winner.

## Suite that ran

HelixTest git tag **v0.1.3** (`1832c043e1679ec283cb2113510ee33684317cce`). The binary printed crate version `0.1.0`. That tag is the Ferrum `VERSIONS.lock` pin. It is not `origin/main`. Profile `ga4gh-drs` does not exist on that tag, so the DRS run used profile `ferrum` with `DRS_URL` set. That profile turns on `strict_drs_checksums` and `supports_beacon_v2`.

Suite pack versions named in `LIMITS.md`: WES 1.1.0, TES 1.1.0, DRS 1.4.0, TRS 2.1.0, htsget 1.3.0, Beacon bundled boolean-response schema labeled Beacon v2.

Images pulled (arm64 where listed):

| Image | Tag | Digest |
|-------|-----|--------|
| `ghcr.io/sapporo-wes/sapporo-service` | 2.2.6 | `sha256:f827d89b47c6d584951f2803e1c682f010e56dc5a00078207cec0cef762cbab4` |
| `ohsucompbio/funnel` | 0.10.1 | `sha256:b3ff5cc039c664bfb687c6dce940813f83d4ed54a0e901881904adb84d8d77a2` |
| `ga4gh/ga4gh-starter-kit-drs` | 0.2.0 | `sha256:f2150a666b8ccab600560797ec486cf52c7818edfa58ac9b658ec03d39212383` |
| `ghcr.io/ega-archive/beacon2-pi-api` | v2.1.2 | `sha256:854ba1c339830ae80b3e38603772024142960ae385c37905c4bd0553607862c5` |
| `mongo` | 5.0.32 | `sha256:7166a36ef06ff8c17ac3884a33528c6a4be66ef0399a6542c42cf3f50299a35d` |

## Sapporo 2.2.6

Command: `helixtest --all --mode ferrum --profile ferrum --only wes --report json --fail-level 0` with `WES_URL` on port **1122**. Exit 1. Result: `results/sapporo-wes-helixtest.json`. Service-info body: `results/sapporo-wes-service-info.json`.

| Call | Result |
|------|--------|
| `GET /service-info` | HTTP 200. `supported_wes_versions` is `["sapporo-wes-2.1.0"]`. `type.version` in the saved body is `sapporo-wes-2.1.0`. |
| `POST /runs` | JSON body. HTTP 500 (`The server encountered an internal error...`). One submit closed the connection before the response finished. |

Request shape the suite sent (`HELIXTEST-BUGS.md` §1–2): `POST {WES}/runs` as JSON. The vendored WES 1.1 OpenAPI lists `multipart/form-data` for that operation. Required fields in the spec are `workflow_type`, `workflow_type_version`, and `workflow_url`. The success case posts `workflow_url` `trs://test-tool/echo/1.0`, `workflow_type` `CWL`, `workflow_type_version` `v1.2`, and `workflow_params.message` `hello-ga4gh`, and expects output field `echo_out`. Other cases use `trs://test-tool/fail/1.0`, `trs://test-tool/cwl-echo/1.0`, and `trs://test-tool/scatter-gather/1.0`. Those URLs are HelixTest fixtures. Submit failed before workflow execution, so those fixtures were not exercised on Sapporo.

## Funnel 0.10.1

Three starts (`REVIEW-PACKET.md`):

1. The generated compose command restart-looped: `unknown command "funnel" for "funnel"` (the entrypoint is already the binary).
2. `server run --config` with the generated YAML exited. Fields `Database.Path`, `Node.Resources.RAM`, and `Docker.Endpoint` were not recognized.
3. `server run --Compute local` with port and resource flags. `funnel version`: git commit `43b289a`, branch `heads/0.10.1`, build date Wed Dec 9 22:10:35 UTC 2020, version string `unknown`. `--Compute docker` was rejected: `unknown compute backend: 'docker'`.

`GET /v1/tasks` returned HTTP 200 and body `{}` (`results/funnel-tasks.json`). Service-info paths returned 404. HelixTest `--only tes` exit 1 (`results/funnel-tes-helixtest.json`). The log `logs/funnel-tes-helixtest.err` shows `GET http://localhost:8000/v1/tasks/{id}`.

| Call | Result |
|------|--------|
| `GET /v1/tasks` | HTTP 200, body `{}`. Reachability passed. |
| `GET /v1/tasks/{id}` with no `view` | HTTP 200. Body keys were `id` and `state` (MINIMAL). Schema check failed: `"executors" is a required property`. The same id with `view=FULL` included `executors` (`HELIXTEST-BUGS.md` §3). |
| Task lifecycle | State `EXECUTOR_ERROR`. Worker log: `exec: "docker": executable file not found in $PATH`. |

The checksum case (`HELIXTEST-BUGS.md` §4) runs `echo hello-tes > /test-data/workflows/outputs/tes_echo_out.txt` in image `alpine` and compares bytes to `test-data/expected/workflows/tes_echo_out.txt.sha256`. This run ended at `EXECUTOR_ERROR` before that comparison.

## Starter Kit DRS 0.2.0

Container log: `config YAML could not be parsed`, then Tomcat on port **4500**. The process served the image-default service-info, not the Community Stack `serviceInfo` block.

| Call | Result |
|------|--------|
| `GET /ga4gh/drs/v1/service-info` | HTTP 200. `type.version` `1.1.0`, `version` `0.1.0`. Body: `results/drs-service-info.json`. Name `GA4GH Starter Kit DRS Service`. |
| `GET /service-info` | HTTP 404. |
| `GET /ga4gh/drs/v1/objects/test-object-1` | HTTP 404 `No DrsObject found by id: test-object-1` (`results/starterkit-drs-helixtest.json`). |
| Invalid object id | HTTP 404. That case passed. |

HelixTest `--profile ferrum --only drs` exit 1. Object, schema, checksum, and Range cases failed because `test-object-1` was absent (`HELIXTEST-BUGS.md` §5). No seed of that id was applied. The suite pack is DRS 1.4.0. This server declared 1.1.0.

## beacon2-pi-api v2.1.2

Mongo and the API started. Four synthetic JSON files were imported, one document each (`generated/community-stack-demo/data/demo/`). HelixTest `--only beacon` exit 1 (`results/beacon-helixtest.json`). `BEACON_URL` resolved to `http://localhost:5050/ga4gh/beacon/v2`.

| Call | Result |
|------|--------|
| `POST /ga4gh/beacon/v2/query` | HTTP 404. All four HelixTest beacon cases failed on that path. |
| `GET` `/info`, `/service-info`, `/configuration`, `/map`, `/entry_types`, `/filtering_terms`, `/g_variants`, `/datasets` under `/ga4gh/beacon/v2` | HTTP 200 (`HELIXTEST-BUGS.md` §6). |
| `POST /g_variants` | HTTP 400 (route exists; empty body rejected). |

Service-info `type.version` is `v2.1.2` (`results/beacon-service-info.json`). The known-variant case, which was not reached, expects referenceName `1`, start `1000`, referenceBases `A`, alternateBases `T`. The Community Stack synthetic record is a different allele (referenceName `22`, start `16050075`, `A`/`G`) and was not queried.

## Not run

From `LIMITS.md` and `REVIEW-PACKET.md`: htsget cases, TRS cases, a seeded Ferrum `make demo` stack, `ga4gh/htsget-refserver:1.5.1` (digest recorded, image not pulled), official GA4GH suites whose versions did not match or whose trees ship reads that were not downloaded, Starter Kit WES, `beacon2-ri-api` as its own process, and Auth / Crypt4GH / E2E / africa / infra against these servers.
