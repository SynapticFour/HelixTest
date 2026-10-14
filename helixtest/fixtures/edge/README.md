# Edge DRS fixture

`test-object-1.txt` is 2240 bytes of ASCII (28 lines). It is not a biological sequence, not a BAM, and not a VCF. `test-object-1.txt.sha256` is its SHA-256.

`seed-drs-test-object.sh` copies that file into a Ferrum edge object directory and inserts DRS id `test-object-1`. The procedure is [docs/edge-profile.md](../../../docs/edge-profile.md).

Required environment: `FERRUM_SQLITE_PATH`, `FERRUM_OBJECTS_DIR`. Optional: `GATEWAY_BASE` (default `http://127.0.0.1:8080`). Stop the edge process before running the script.
