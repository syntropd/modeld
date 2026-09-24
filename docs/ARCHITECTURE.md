# Architecture Specification: modeld

## 1. System Purpose & Functional Scope

`modeld` operates as an unprivileged Linux system daemon that provides Content-Addressable Storage (CAS), cryptographic verification, and zero-copy file descriptor sharing for machine learning model weights.

## 2. Core Functional Components

```
Client Applications (inferenced, runtimed)
       │
       │  1. Varlink RPC (io.syntrop.Model1)
       ▼
   modeld (Varlink Server)
       │
       │  2. SCM_RIGHTS Descriptors (sealed memfds)
       ▼
Linux Kernel VFS (memfd_create + FICLONE)
       │
       ▼
/var/lib/models/cas/blobs/sha256/<hash>
```

### 2.1 Content-Addressable Storage (CAS)
Model weights are stored strictly by their SHA-256 cryptographic digest:
- Storage path: `/var/lib/models/cas/blobs/sha256/<64-char-hex>`
- Human tags: `/var/lib/models/tags/<name>/<variant>` (each segment validated against `[A-Za-z0-9._-+]`)
- Pinned records: `/var/lib/models/pinned/<digest>`

Writes execute atomically: incoming data is streamed to `/var/lib/models/cas/incoming/stage-*` while SHA-256 is updated inline, verified against the expected digest if supplied, and moved via atomic filesystem rename (`renameat2`). The hash is computed during the write loop, not in a second disk pass.

### 2.2 Reflink Cloning & Deduplication
On copy-on-write filesystems (Btrfs, XFS), `modeld` provisions working copies using `ioctl(FICLONE)`. This achieves instantaneous file cloning with zero storage duplication. If the host filesystem does not support extents, it falls back to standard file copying.

### 2.3 Zero-Copy SCM_RIGHTS Descriptor Handoff

When authorized daemons (such as `inferenced`) request a model:
1. `modeld` resolves the identifier and opens the immutable blob.
2. `modeld` streams the blob into a `memfd_create` descriptor with `MFD_ALLOW_SEALING`, then applies the seals `WRITE | SHRINK | GROW | SEAL` so the kernel enforces content immutability.
3. `modeld` transmits the sealed descriptor across a Unix domain socket using `SCM_RIGHTS` ancillary data.
4. The consumer receives the descriptor and executes `mmap(MAP_SHARED)`. Zero bytes copy through userspace memory buffers.

The seal set is the consumer-side guarantee that the recipient cannot mutate, shrink, grow, or further-seal the descriptor. Earlier revisions of this document described `fs-verity` sealing; that mechanism remains a future option but the current implementation uses memfd seals for portability across filesystems.

### 2.4 Security & Pickle Rejection
`modeld` enforces strict format safety:
- Permitted: GGUF (v2, v3), SafeTensors.
- Permitted by magic-byte sniff (no dedicated parser): ONNX (protobuf).
- Banned: Python pickle serialized files (`.pt`, `.bin`, `.pkl`, `.pickle`, `.joblib`).
- Magic opcode check: rejects stream headers starting with Python pickle opcodes (`0x80 0x02..0x05`) before any extension check.
- Tag names are validated against `[A-Za-z0-9._-+]`; `.`, `..`, `/`, `\`, NUL, and shell metacharacters are rejected.

### 2.5 Identifier Validation
Varlink methods that accept user-supplied identifiers (`Inspect`, `Pin`, `Unpin`) distinguish three outcomes:
- Found → `Ok` with parameters
- Well-formed but unknown → `io.syntrop.Model1.NoSuchModel`
- Rejected as unsafe → `io.syntrop.Model1.InvalidIdentifier`

The third case is not collapsed into `NoSuchModel`; that would let a malicious client probe the registry for valid tag names without revealing the validation rejection.

