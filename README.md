# modeld

> **Unprivileged Content-Addressable Model Store and Zero-Copy Descriptor Broker in Pure Rust.**
> Solves the AI storage and memory duplication problem on Linux by providing cryptographic CAS deduplication, sealed memfd descriptor handoff, and `SCM_RIGHTS` zero-copy descriptor sharing.

[![Platform: Linux / systemd](https://img.shields.io/badge/Platform-Linux%20%2F%20systemd-red.svg)](https://systemd.io/)
[![Language: Rust](https://img.shields.io/badge/Language-Pure%20Rust-orange.svg)](https://www.rust-lang.org/)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Zero C Dependencies](https://img.shields.io/badge/Dependencies-Zero%20C%20Libs-green.svg)](#architecture)
[![Version: 0.2.0](https://img.shields.io/badge/Version-0.2.0-blue.svg)](CHANGELOG.md)

---

## The Problem: AI Weight Duplication on Linux

Modern AI runtimes (Ollama, vLLM, HuggingFace, Whisper) behave like isolated package managers:
1. **Massive Disk Bloat**: Each framework redownloads 4 GB–70 GB model weights into hidden dotfiles (`~/.cache`, `~/.ollama`), exhausting disk capacity.
2. **Buffer Memory Thrashing**: Passing models between daemons copies gigabytes through userspace memory buffers, causing RAM spikes and cache eviction.
3. **Execution Vulnerabilities**: Many frameworks silently deserialize untrusted Python pickle formats (`.bin`, `.pt`), introducing arbitrary code execution vulnerabilities.

---

## The Solution: modeld

`modeld` operates as an unprivileged system daemon:

* **Content-Addressable Storage (CAS)**: Models are stored strictly by their SHA-256 cryptographic digest in `/var/lib/models/cas/`.
* **Zero-Copy Reflink Clones**: Leverages Btrfs/XFS `ioctl(FICLONE)` for instant, zero-byte file duplication.
* **Sealed Memfd Descriptor Handoff**: Streams each blob into a `memfd_create` descriptor with `WRITE | SHRINK | GROW | SEAL` applied, then hands the sealed FD to consumer daemons (e.g. `inferenced`) via `SCM_RIGHTS`. Consumers execute `mmap(MAP_SHARED)` directly. The kernel enforces that the recipient cannot mutate, shrink, grow, or further-seal the descriptor — zero bytes copy through userspace memory.
* **Format Sanitization**: Pure Rust stream header parsers for GGUF and SafeTensors. Strictly bans Python pickle files.
* **Native Systemd Citizen**: Socket activated (`$LISTEN_FDS`), unprivileged sandboxing (`ai.slice`), `Type=notify`, and `WatchdogSec=15`.
* **Companion CLI (`modelctl`)**: Complete operator tooling for listing, inspecting, pinning, and pruning models.

---

## Quickstart

### 1. Build and Run Tests
```bash
cargo test --workspace
```

### 2. Install
```bash
sudo ./install/install.sh
```

### 3. Usage with `modelctl`
```bash
# Import a model file into the CAS store
modelctl import ./weights.gguf --tag=llama3.2:1b

# List cached models
modelctl list

# Inspect format metadata and context window
modelctl inspect llama3.2:1b

# Pin emergency triage model so it is never pruned
modelctl pin llama3.2:1b

# Prune unpinned models down to 10 GiB. --max-bytes is REQUIRED:
# `modelctl prune` with no flag is rejected to prevent accidental
# deletion of every unpinned model.
modelctl prune --max-bytes=10737418240
```

---

## Security Model

| Concern | Mitigation |
|---------|-----------|
| Path traversal via tag names | `TagRegistry` validates every `name`/`variant` against `[A-Za-z0-9._-+]`; rejects `.`, `..`, `/`, `\`, NUL |
| Pickle deserialization RCE | `validate_file_safety` blocks `.pt`, `.bin`, `.pkl`, `.pickle`, `.joblib`; `detect_safe_format` rejects `0x80 0x02..0x05` magic early |
| CAS blob tampering | Blobs are content-addressed by SHA-256; the digest is the path |
| Consumer-side mutation of received FDs | memfd `WRITE | SHRINK | GROW | SEAL` locks content immutability |
| DoS via oversized SafeTensors header | `header_len` capped at 8 MiB; allocations stay bounded |
| DoS via malformed GGUF | Array skip is seek-based, not element-by-element; truncations surface as errors instead of silent partial metadata |
| Identifiers from untrusted peers | `Inspect`, `Pin`, `Unpin` return `io.syntrop.Model1.InvalidIdentifier` for unsafe segments instead of a misleading `NoSuchModel` |

---

## Documentation

* [System Architecture](docs/ARCHITECTURE.md): Storage layout, zero-copy descriptor handoff, and security validation.
* [Varlink Specification](docs/VARLINK_SPEC.md): `io.syntrop.Model1` interface definitions and method descriptions.
* [CLI Reference](docs/CLI_REFERENCE.md): Detailed subcommands and options for `modelctl`.
* [Changelog](CHANGELOG.md): Release history and breaking-change notices.

---

## License

[Apache 2.0](LICENSE) © Syntropd Authors

