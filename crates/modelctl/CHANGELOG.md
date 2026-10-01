# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.2](https://github.com/syntropd/modeld/compare/v0.3.1...v0.3.2) - 2026-10-01

### Added

- *(envelope)* implement hardware envelope sizing engine and modelctl bootstrap
- *(modelctl)* add Qwen 2.5 Coder aliases for 1.5B and 7B models
- *(format)* add safetensors support, format security rejection, and refactor pull resolve
- *(modeld,modelctl)* implement syn pull and modelctl pull streaming downloader with CAS commit and register

### Fixed

- *(pull, envelope)* prevent parallel staging collision, fix quant suffix matching, and point Qwen to unsharded GGUFs
- *(ci)* track Cargo.lock and format code for CI
- *(modeld,modelctl)* resolve staging leak, dual CAS layout, and GGUF registration

### Other

- apply cargo fmt and split envelope profiles into profiles.rs for page rule compliance
- *(modeld)* bump workspace and crates to v0.3.2
