# Verification record

Verified locally on Linux x64 against Meilisearch **v1.56.2**, image digest
`sha256:9600db22be3fcbf3d506176d3a73dd70a65c5719d9aa1d2837355b75afb8f4fb`.
The native host was Tabularis **0.32.0**, Core feature commit
`6513f879ee227c92ff31ad87f2732e875a958553`, built with `e2e-testing`.
Only the standard executable driver protocol is used; no provider-prototype UI
or in-Core Meilisearch implementation is required.

## Automated checks

- `cargo fmt --check` passed.
- `cargo test --locked`: 16 unit tests passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo build --release --locked` passed.
- `python3 tests/live.py`: 8 live regression tests passed against 205 synthetic
  documents and an empty index with a custom primary key.
- Authentication rejects an invalid key. Pagination checks two distinct pages,
  exact totals and an empty page. Native search checks typo tolerance, filters,
  sorting and no matches. CRUD checks persisted data, preserved fields,
  duplicate insert rejection, missing-document update rejection, primary-key
  mutation rejection and deletion. An invalid vector dimension reproduces a
  failed asynchronous indexing task and verifies it is returned as an error.

## Real native app

The test used an isolated config/data directory and file-backed test secret store.
The user's saved connections were not changed. Through the actual desktop UI:

1. Selected the installed Meilisearch driver in Add Connection, entered localhost
   credentials, ran Test Connection successfully and saved the connection.
2. Connected, discovered `movies` and `empty_index`, and opened the native table grid.
3. Paged forward, checked the final five documents out of 205 and returned to page one.
4. Edited a title cell and submitted the change. The Meilisearch REST API confirmed
   the persisted title and preserved nested fields.
5. Inserted document `9001` through the grid and submitted it. The REST API confirmed
   its data. Selected that row, deleted it and submitted; REST returned HTTP 404.
6. Ran `@meili-search` in the normal editor with `q: matrx` and `year = 1999`;
   exactly the Matrix document appeared in the result grid. A no-match query
   completed successfully without rows. Unsupported SQL WHERE returned a clear error.

`screenshots/browse.png` and `screenshots/search.png` are unmodified captures of
that real app window with synthetic data.

The seed and all eight live checks also passed against a second, completely fresh
container. The initial CI failure for deleting an absent index was a fixture bug:
the seeder now checks index existence before enqueueing deletion.

## Findings and limits

The first test host used a different compiled frontend and produced an empty
editor; verification was repeated with a freshly built, matching host/frontend.
Native testing exposed an incorrect `is_primary_key` metadata field; it was fixed
to the host's `is_pk` field before final verification.

WebDriver-generated editor input produced Monaco clipboard `NotAllowedError`
and canceled async-operation messages. These did not prevent the verified queries
or persisted CRUD actions. No claim is made that the complete host console is warning-free.

## Native platform release gate

The Native packages workflow builds and runs unit tests on Windows x64,
macOS Intel, macOS Apple Silicon, Linux x64 and Linux ARM64. Each job starts
Meilisearch v1.56.2 in a disposable directory, seeds 205 demo documents and runs
the eight live JSON-RPC tests against both the built executable and the extracted
ZIP. A machine-readable per-platform verification report records the archive
hash, host architecture and both live-test passes. Publication checks every
report and hash; failed or canceled jobs cannot publish a partial release.

The Apple Silicon driver is ARM64-native. The upstream macOS Meilisearch test
server is x64 and runs through Rosetta on that runner. This does not emulate the
driver. Native platform builds and stdio/HTTP live tests are distinct from a
Tabularis desktop GUI E2E run, which remains verified on Linux x64 only.

Custom TLS/CA environments, vector UI and production clusters are not tested.
The historical search toolbar remains excluded. See README for query/sample
limits and non-transactional write behavior.
