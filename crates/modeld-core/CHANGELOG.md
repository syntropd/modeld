# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0](https://github.com/syntropd/modeld/compare/v0.3.1...v0.4.0) - 2026-10-02

### Added

- *(format)* implement visual Safetensors inspection and modelctl visual/lora pull
- *(envelope)* implement hardware envelope sizing engine and modelctl bootstrap
- *(format)* add safetensors support, format security rejection, and refactor pull resolve

### Fixed

- *(pull, envelope)* prevent parallel staging collision, fix quant suffix matching, and point Qwen to unsharded GGUFs
- *(ci)* track Cargo.lock and format code for CI
- *(modeld,modelctl)* resolve staging leak, dual CAS layout, and GGUF registration

### Other

- apply cargo fmt and split envelope profiles into profiles.rs for page rule compliance
