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
       │  2. SCM_RIGHTS Descriptors
       ▼
Linux Kernel VFS (memfd_create + fs-verity + FICLONE)
       │
       ▼
/var/lib/models/cas/blobs/sha256/<hash>
```

### 2.1 Content-Addressable Storage (CAS)
Model weights are stored strictly by their SHA-256 cryptographic digest:
- Storage path: `/var/lib/models/cas/blobs/sha256/<64-char-hex>`
- Human tags: `/var/lib/models/tags/<name>/<variant>`
- Pinned records: `/var/lib/models/pinned/<digest>`

Writes execute atomically: incoming data is buffered to `/var/lib/models/cas/incoming/stage-*`, verified against the expected hash, and moved via atomic filesystem rename (`renameat2`).

### 2.2 Reflink Cloning & Deduplication
On copy-on-write filesystems (Btrfs, XFS), `modeld` provisions working copies using `ioctl(FICLONE)`. This achieves instantaneous file cloning with zero storage duplication. If the host filesystem does not support extents, it falls back to standard file copying.

### 2.3 Zero-Copy SCM_RIGHTS Descriptor Handoff
When authorized daemons (such as `inferenced`) request a model:
1. `modeld` resolves the identifier and opens the immutable blob.
2. `modeld` transmits the open file descriptor across a Unix domain socket using `SCM_RIGHTS` ancillary data.
3. The consumer receives the descriptor and executes `mmap(MAP_SHARED)`. Zero bytes copy through userspace memory buffers.

### 2.4 Security & Pickle Rejection
`modeld` enforces strict format safety:
- Permits: GGUF (v2, v3), SafeTensors, ONNX.
- Banned: Python pickle serialized files (`.pt`, `.bin`, `.pkl`).
- Magic opcode checking: Rejects stream headers starting with Python pickle opcodes (`0x80`).
