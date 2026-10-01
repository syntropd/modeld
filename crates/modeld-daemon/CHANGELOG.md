# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.2](https://github.com/syntropd/modeld/compare/v0.3.1...v0.3.2) - 2026-10-01

### Added

- *(format)* add safetensors support, format security rejection, and refactor pull resolve
- *(modeld,modelctl)* implement syn pull and modelctl pull streaming downloader with CAS commit and register

### Fixed

- *(ci)* track Cargo.lock and format code for CI
- *(register)* strip .safetensors suffix and add security rejection and pull tests
- *(modeld,modelctl)* resolve staging leak, dual CAS layout, and GGUF registration

### Other

- *(modeld)* bump workspace and crates to v0.3.2
