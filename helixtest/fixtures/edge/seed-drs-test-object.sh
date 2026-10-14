#!/usr/bin/env bash
# Register DRS object id test-object-1 from the synthetic fixture in this directory.
#
# Ferrum's POST /ga4gh/drs/v1/ingest/file assigns a ULID. HelixTest requests
# the id test-object-1, so this script writes that id into the edge SQLite
# database and copies the bytes into the local object store.
#
# Stop the edge process first. SQLite may be locked while it is running, and
# a running process will not see rows written under it. Start it again after
# this script prints the seeded line.
#
# Required environment (no default path; do not point this at a home directory
# baked into the script):
#   FERRUM_SQLITE_PATH   edge SQLite file created by a previous edge start
#   FERRUM_OBJECTS_DIR   local object storage root (storage base path)
# Optional:
#   GATEWAY_BASE         default http://127.0.0.1:8080
#
# The fixture is ASCII text, longer than 1024 bytes, so DRS Range bytes=0-1023
# can be 206. It is not a BAM or VCF. htsget reads and variants tickets still
# need those objects. Checksum type is sha256, which is the string HelixTest
# compares. No checksum-status row is written, so the server is not told the
# checksum is pending.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURE="${ROOT}/test-object-1.txt"
SIDECAR="${ROOT}/test-object-1.txt.sha256"

if [[ -z "${FERRUM_SQLITE_PATH:-}" || -z "${FERRUM_OBJECTS_DIR:-}" ]]; then
  echo "set FERRUM_SQLITE_PATH and FERRUM_OBJECTS_DIR" >&2
  exit 1
fi
GATEWAY_BASE="${GATEWAY_BASE:-http://127.0.0.1:8080}"
GATEWAY_BASE="${GATEWAY_BASE%/}"

case "${GATEWAY_BASE}" in
  http://*|https://*) ;;
  *)
    echo "GATEWAY_BASE must start with http:// or https://" >&2
    exit 1
    ;;
esac
case "${GATEWAY_BASE}" in
  *"'"*)
    echo "GATEWAY_BASE must not contain a single quote" >&2
    exit 1
    ;;
esac

if [[ ! -f "${FERRUM_SQLITE_PATH}" ]]; then
  echo "SQLite database not found. Start the edge binary once so it migrates, stop it, then re-run." >&2
  exit 1
fi
if [[ ! -f "${FIXTURE}" || ! -f "${SIDECAR}" ]]; then
  echo "missing fixture or sha256 sidecar next to this script" >&2
  exit 1
fi
if ! command -v sqlite3 >/dev/null 2>&1; then
  echo "sqlite3 is required" >&2
  exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
  ACTUAL="$(sha256sum "${FIXTURE}" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
  ACTUAL="$(shasum -a 256 "${FIXTURE}" | awk '{print $1}')"
else
  echo "need sha256sum or shasum" >&2
  exit 1
fi
EXPECTED="$(awk '{print $1}' "${SIDECAR}")"
if [[ "${ACTUAL}" != "${EXPECTED}" ]]; then
  echo "fixture sha256 does not match the sidecar" >&2
  exit 1
fi

SIZE="$(wc -c < "${FIXTURE}" | tr -d '[:space:]')"
if [[ "${SIZE}" -le 1024 ]]; then
  echo "fixture must be longer than 1024 bytes" >&2
  exit 1
fi

install -d "${FERRUM_OBJECTS_DIR}/drs"
cp "${FIXTURE}" "${FERRUM_OBJECTS_DIR}/drs/test-object-1"

STREAM_URL="${GATEWAY_BASE}/ga4gh/drs/v1/objects/test-object-1/stream"
ACCESS_JSON="$(printf '{"url":"%s"}' "${STREAM_URL}")"

sqlite3 "${FERRUM_SQLITE_PATH}" <<SQL
BEGIN;
DELETE FROM drs_bundle_contents WHERE bundle_id = 'test-object-1' OR object_id = 'test-object-1';
DELETE FROM drs_object_metadata WHERE object_id = 'test-object-1';
DELETE FROM drs_checksums WHERE object_id = 'test-object-1';
DELETE FROM drs_access_methods WHERE object_id = 'test-object-1';
DELETE FROM storage_references WHERE object_id = 'test-object-1';
DELETE FROM drs_objects WHERE id = 'test-object-1';
INSERT INTO drs_objects (id, name, description, mime_type, size, is_bundle, aliases)
VALUES (
  'test-object-1',
  'HelixTest edge synthetic object',
  'Synthetic bytes for DRS id test-object-1. Not a biological sequence. Not a BAM or VCF.',
  'text/plain',
  ${SIZE},
  0,
  '[]'
);
INSERT INTO drs_checksums (object_id, type, checksum)
VALUES ('test-object-1', 'sha256', '${ACTUAL}');
INSERT INTO drs_access_methods (object_id, type, access_id, access_url, headers)
VALUES (
  'test-object-1',
  'https',
  'access-test-object-1',
  '${ACCESS_JSON}',
  '[]'
);
INSERT INTO storage_references (object_id, storage_backend, storage_key, is_encrypted)
VALUES ('test-object-1', 'local', 'drs/test-object-1', 0);
COMMIT;
SQL

echo "seeded test-object-1 sha256=${ACTUAL} bytes=${SIZE}"
echo "stream ${STREAM_URL}"
echo "Restart the edge process so it opens this database."
