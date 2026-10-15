# Edge profile

Profile file: `helixtest/profiles/edge.toml`.

This profile is for the Ferrum **edge** binary: one process, SQLite, the surfaces that binary serves. It is not the full gateway profile (`profiles/ferrum.toml`) and it is not `--mode ferrum+infra`.

HelixTest does not start the binary. A green or red run is a technical signal, not GA4GH certification.

## What runs

Enabled services: `drs`, `beacon`, `htsget`, `crypt4gh`.

| Service | What the profile turns on |
|---------|---------------------------|
| DRS | Checksum check is on (`strict_drs_checksums`). Object id is `test-object-1`. |
| Beacon | `supports_beacon_v2` is on. The known-variant case still expects referenceName `1`, start `1000`, referenceBases `A`, alternateBases `T`. |
| htsget | Base URL is set, so the suite does not skip htsget for lack of a URL. |
| Crypt4GH | The HTTP checks stay off unless `HELIXTEST_FEATURE_CRYPT4GH_REWRAP` / `HELIXTEST_FEATURE_CRYPT4GH_PLAIN` are set. A skip is not a pass. |

Scatter/gather stays off. WES is not in this profile.

## What is skipped, and why

WES, TES, TRS, and Auth are not in `enabled_services`. The runner lists them under **Skipped services** with the reason `skipped by profile or --only filter`. They do not appear in the service table, so they are not reported as passed.

That skip is deliberate:

- The 2026-10 edge run recorded HTTP 503 `service disabled via hot-reload config` for WES, TES, and TRS, and HTTP 503 for Auth `/service-info`. Evidence: `evidence/2026-10/a-lite-evidence-run/part2/helixtest-ferrum-edge.table.txt` (exit 1). DRS `GET .../objects/test-object-1` was HTTP 404 `object not found: test-object-1`.
- Passports are the `--mode ferrum+infra` checks. This profile does not run them and does not mark them passed.
- Age is an in-process check, not a call to the edge binary, so it is disabled here.
- E2E drives TRS and WES, which this binary does not serve, so it is disabled here.

Their URLs in the profile are `http://127.0.0.1:9`. If a later change enables them without editing those URLs, the call fails closed.

## Run

```bash
helixtest --all --profile edge --report table
```

Default origin is `http://127.0.0.1:8080`. If the process listens elsewhere, set `DRS_URL`, `BEACON_URL`, and `HTSGET_URL` to that origin's `/ga4gh/drs/v1`, `/ga4gh/beacon/v2`, and `/ga4gh/htsget/v1`.

Beacon's known-variant case fails when that allele is absent (`exists=false` in the 2026-10 edge table). Seeding that row is separate from the DRS object below. The published-edge CI workflow inserts `chr1:1000 A>T` into `beacon_variants`; this profile does not do that insert.

htsget reads tickets still expect a reads-class object (default id `test-object-1`, MIME a reads type). The synthetic file below is `text/plain`. After this seed, DRS can see the object. htsget reads and variants tickets still fail until a BAM-class object and `demo-sample-vcf` (or `HTSGET_VARIANTS_OBJECT_ID`) exist. This document does not call that a pass.

## Seed DRS `test-object-1`

The public ingest route assigns its own id. HelixTest requests `test-object-1`, so the seed writes that id.

Fixture: `helixtest/fixtures/edge/test-object-1.txt` (2240 ASCII bytes, 28 lines). Sidecar: `test-object-1.txt.sha256`. Script: `helixtest/fixtures/edge/seed-drs-test-object.sh`.

1. Start the edge binary once so it creates and migrates the SQLite file. Stop it.
2. Set `FERRUM_SQLITE_PATH` to that file and `FERRUM_OBJECTS_DIR` to the local object storage root.
3. Optionally set `GATEWAY_BASE` (default `http://127.0.0.1:8080`).
4. Run the script. It checks the sidecar hash, copies the file to `${FERRUM_OBJECTS_DIR}/drs/test-object-1`, and inserts:

   - `drs_objects.id = test-object-1`, `mime_type = text/plain`, `size` = the file length
   - `drs_checksums` type `sha256` (the string the DRS checksum case compares)
   - `drs_access_methods.access_url` = `{"url":"<GATEWAY_BASE>/ga4gh/drs/v1/objects/test-object-1/stream"}`
   - `storage_references` backend `local`, key `drs/test-object-1`

5. Start the edge binary again.

No checksum-status metadata row is written. A `pending` status would hide the checksum from the object JSON and the checksum case would fail.

The script does not download anything and does not read a patient file. It only copies the fixture in this repository.
