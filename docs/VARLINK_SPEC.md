# Varlink Interface Specification: io.syntrop.Model1

## 1. Interface Overview

The `io.syntrop.Model1` interface exposes model querying, inspection, pinning, and storage quota management over Unix domain sockets.

Socket Location: `/run/syntrop/io.syntrop.Model1`

## 2. Interface Definition Text

```varlink
interface io.syntrop.Model1

type ModelEntry (
  id: string,
  digest: string,
  name: ?string,
  tag: ?string,
  size_bytes: int,
  pinned: bool,
  format: string
)

method List() -> (models: []ModelEntry)
method Inspect(id: string) -> (info: ModelEntry, metadata: ?string)
method Pin(id: string) -> ()
method Unpin(id: string) -> ()
method Prune(max_bytes: int) -> (reclaimed_bytes: int)
method GetStorageStats() -> (total_bytes: int, model_count: int, pinned_count: int)

error NoSuchModel(id: string)
error InvalidIdentifier(id: string)
error InvalidParameter(parameter: string, reason: string)
error OperationFailed(reason: string)
```

## 3. Method Specifications

### 3.1 `List()`
Returns an array of all registered models and tags currently stored in the Content-Addressable Storage repository.

### 3.2 `Inspect(id: string)`
Resolves a human tag (e.g. `llama3.2:1b`) or SHA-256 digest and returns full metadata (format, size, pinned status, tensor counts).

Errors:
- `InvalidIdentifier` — the supplied `id` fails segment validation (rejected characters, empty, `.`, `..`).
- `NoSuchModel` — the identifier is well-formed but no model matches.

### 3.3 `Pin(id: string)`
Marks a model as immune to automated LRU cache reclamation. Pinned models will never be evicted during disk pressure.

Errors: same as `Inspect`.

### 3.4 `Unpin(id: string)`
Removes the protection pin, returning the model to standard LRU eviction policy.

Errors: same as `Inspect`.

### 3.5 `Prune(max_bytes: int)`
Triggers manual storage reclamation. Unpinned models are pruned in least-recently-used order until total storage is below `max_bytes`.

Errors:
- `InvalidParameter` — `max_bytes` is zero or negative. A zero value would delete every unpinned model and is rejected by the daemon as defense in depth against direct Varlink callers.

### 3.6 `GetStorageStats()`
Returns aggregate counts and byte totals for the CAS store.

## 4. Identifier Validation Rules

Identifiers passed to `Inspect`, `Pin`, and `Unpin` may be:
1. A 64-character lowercase or uppercase hex SHA-256 digest, or
2. A `name:tag` pair where both segments are validated against `[A-Za-z0-9._-+]`.

The following inputs are rejected with `InvalidIdentifier` rather than `NoSuchModel`:
- Empty segments.
- `.` or `..`.
- Embedded `/`, `\`, or NUL bytes.
- Any byte outside `[A-Za-z0-9._-+]`.

