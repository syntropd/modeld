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
error OperationFailed(reason: string)
```

## 3. Method Specifications

### 3.1 `List()`
Returns an array of all registered models and tags currently stored in the Content-Addressable Storage repository.

### 3.2 `Inspect(id: string)`
Resolves a human tag (e.g. `llama3.2:1b`) or SHA-256 digest and returns full metadata (format, size, pinned status, tensor counts).

### 3.3 `Pin(id: string)`
Marks a model as immune to automated LRU cache reclamation. Pinned models will never be evicted during disk pressure.

### 3.4 `Unpin(id: string)`
Removes the protection pin, returning the model to standard LRU eviction policy.

### 3.5 `Prune(max_bytes: int)`
Triggers manual storage reclamation. Unpinned models are pruned in least-recently-used order until total storage is below `max_bytes`.
