This directory holds deterministic test data for HelixTest (GA4GH conformance suite).

Structure:

- `workflows/` – workflow source files and mock output locations
  - `cwl/`, `wdl/`, `nextflow/`
  - `outputs/` – deterministic outputs produced by mock services
- `inputs/` – small FASTQ-like inputs and parameter JSON
- `expected/` – expected outputs and checksum files

Mocks in `docker/docker-compose.yml` are expected to read/write from these paths.

