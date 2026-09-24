# CLI Reference: modelctl

`modelctl` is the operator utility for managing the `modeld` Content-Addressable Storage service.

## 1. Global Options

* `--socket <PATH>`: Override Varlink socket path (default: `/run/syntrop/io.syntrop.Model1`).
* `--storage-path <PATH>`: Override CAS storage root (default: `/var/lib/models`).
* `--json`: Output structured JSON for automation and shell pipelines.
* `-h, --help`: Display help and usage options.
* `-V, --version`: Display program version.

## 2. Subcommands

### 2.1 `modelctl list` (alias: `ls`)
Displays all models in the repository.
```bash
modelctl list
modelctl list --json
```

### 2.2 `modelctl inspect <ID>`
Displays detailed header metadata, digest, tensor count, and pinning status.
```bash
modelctl inspect llama3.2:1b
```

### 2.3 `modelctl import <PATH> [--tag=<NAME:TAG>]`
Imports a local file directly into the Content-Addressable Storage store.
```bash
modelctl import ./weights.gguf --tag=qwen2.5:7b
```

### 2.4 `modelctl pin <ID>` & `modelctl unpin <ID>`
Toggles eviction protection on a specific model.
```bash
modelctl pin llama3.2:1b
modelctl unpin llama3.2:1b
```

### 2.5 `modelctl prune [--max-bytes=<BYTES>]`
Evicts unpinned models to reclaim host storage.
```bash
modelctl prune --max-bytes=10737418240  # Reclaim down to 10 GiB
```

### 2.6 `modelctl completions <SHELL>`
Generates native shell completion scripts to standard output.
```bash
source <(modelctl completions bash)
```
