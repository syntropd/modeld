# Changelog

All notable changes to modeld are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-09-24

### Changed

- **Breaking:** `SafeTensors` JSON header length cap tightened from
  33 MiB to 8 MiB. Real SafeTensors headers are well under 1 MiB; the
  new cap is a tighter DoS guard against attacker-controlled
  `header_len` values. Files with headers between 8 MiB and 33 MiB that
  were previously accepted are now rejected with `InvalidFormat`.
- `modelctl prune --max-bytes` is now required. The bare `modelctl
  prune` previously sent `max_bytes=0`, which silently deleted every
  unpinned model; the CLI rejects missing or zero values, and the daemon
  rejects `max_bytes=0` with `InvalidParameter` as defense in depth.
- `modeld-daemon` Varlink handlers for `Inspect`, `Pin`, and `Unpin` now
  return `io.syntrop.Model1.InvalidIdentifier` for identifiers that fail
  segment validation, instead of the misleading `NoSuchModel`. This also
  wires up the previously-unused `InvalidIdentifier` error declared in
  the Varlink specification.

### Fixed

- `sd_notify` no longer silently fails on the abstract namespace path
  that stock systemd uses. Implementation now builds the `sockaddr_un`
  via `rustix::net::sendto_unix` so the kernel receives the correct
  `addrlen` for both abstract and filesystem targets.
- `TagRegistry::set_tag` now rejects path-traversal attempts
  (`../../etc`, `..`, `/`, `\`, NUL, shell metacharacters, non-ASCII)
  via the new `validate_tag_segment` helper, which is also applied to
  `get_tag`, `remove_tag`, and `resolve`.
- `parse_gguf_header` no longer silently truncates on mid-header EOF
  or returns garbage after a metadata array with more than 1024 elements.
  Arrays of fixed-width elements are now seek-skipped in a single
  syscall; nested and string arrays are bounded by a 64 KiB safety cap.
- `detect_safe_format` no longer accepts random binary blobs whose byte 8
  happens to be `{`. SafeTensors detection now requires the JSON head to
  contain only valid JSON bytes (no control characters) within the
  first 16 bytes of the stream.
- `CasStore::store_blob` no longer reads the staged file back from disk
  to compute the SHA-256 digest. The hash is now streamed inline during
  the write loop, halving I/O for multi-gigabyte imports.
- `fd_server` now streams the CAS blob into a sealed memfd and sends
  the sealed descriptor via SCM_RIGHTS, replacing the previous plain
  read-only FD handoff that did not deliver the
  ARCHITECTURE.md-promised immutability guarantee. The handoff request
  loop is now NUL-framed (matching the Varlink path) and bounded at
  64 KiB to prevent hostile clients from OOMing the daemon.

### Added

- `create_sealed_memfd_from_file` helper in `modeld-core::descriptor`
  for streaming a file into a sealed memfd.
- Unit and edge tests for: tag segment validation, GGUF array
  truncation resilience, SafeTensors oversized-header rejection, sealed
  memfd content and immutability, sd_notify address construction,
  Varlink `InvalidIdentifier` translation.

## [0.1.0] - 2026-09-23

### Added

- Initial release: unprivileged Content-Addressable Model Store with
  SHA-256 digests, FICLONE reflink copying, SCM_RIGHTS descriptor
  handoff, Varlink IPC (`io.syntrop.Model1`), and the `modelctl`
  operator CLI.
