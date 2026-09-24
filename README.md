# modeld

> **Unprivileged Content-Addressable Model Store and Zero-Copy Descriptor Broker in Pure Rust.**  
> Solves the AI storage and memory duplication problem on Linux by providing cryptographic CAS deduplication, kernel `fs-verity` sealing, and `SCM_RIGHTS` zero-copy descriptor sharing.

[![Platform: Linux / systemd](https://img.shields.io/badge/Platform-Linux%20%2F%20systemd-red.svg)](https://systemd.io/)
[![Language: Rust](https://img.shields.io/badge/Language-Pure%20Rust-orange.svg)](https://www.rust-lang.org/)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Zero C Dependencies](https://img.shields.io/badge/Dependencies-Zero%20C%20Libs-green.svg)](#architecture)

---

## The Problem: AI Weight Duplication on Linux

Modern AI runtimes (Ollama, vLLM, HuggingFace, Whisper) behave like isolated package managers:
1. **Massive Disk Bloat**: Each framework redownloads 4GB–70GB model weights into hidden dotfiles (`~/.cache`, `~/.ollama`), exhausting disk capacity.
2. **Buffer Memory Thrashing**: Passing models between daemons copies gigabytes through userspace memory buffers, causing RAM spikes and cache eviction.
3. **Execution Vulnerabilities**: Many frameworks silently deserialize untrusted Python pickle formats (`.bin`, `.pt`), introducing arbitrary code execution vulnerabilities.

---

## The Solution: modeld

`modeld` operates as an unprivileged system daemon:

* **Content-Addressable Storage (CAS)**: Models are stored strictly by their SHA-256 cryptographic digest in `/var/lib/models/cas/`.
* **Zero-Copy Reflink Clones**: Leverages Btrfs/XFS `ioctl(FICLONE)` for instant, zero-byte file duplication.
* **Kernel Sealing & Descriptors**: Passes open, sealed read-only file descriptors to consumer daemons (e.g. `inferenced`) via `SCM_RIGHTS`. Consumer daemons execute `mmap(MAP_SHARED)` directly. Zero bytes copy through userspace memory.
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

# Prune unpinned models down to 10 GiB
modelctl prune --max-bytes=10737418240
```

---

## Documentation

* [System Architecture](docs/ARCHITECTURE.md): Storage layout, zero-copy descriptor handoff, and security validation.
* [Varlink Specification](docs/VARLINK_SPEC.md): `io.syntrop.Model1` interface definitions and method descriptions.
* [CLI Reference](docs/CLI_REFERENCE.md): Detailed subcommands and options for `modelctl`.

---

## License

[Apache 2.0](LICENSE) © Syntropd Authors
