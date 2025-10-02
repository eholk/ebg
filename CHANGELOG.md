# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.5](https://github.com/eholk/ebg/compare/v0.5.4...v0.5.5) - 2025-10-02

### Other

- Bump toml from 0.8.23 to 0.9.7
- Bump thiserror from 2.0.16 to 2.0.17
- Bump clap from 4.5.47 to 4.5.48
- Bump tempfile from 3.21.0 to 3.23.0
- Bump indicatif from 0.17.11 to 0.18.0
- cargo update
- update rust version
- Fail workflow if release tag cannot be retrieved instead of falling back to main
- Pass release tag to Docker workflow to ensure consistent builds
- Improve error handling and logging for Docker workflow trigger
- Add automatic Docker workflow trigger to release-plz workflow

## [0.5.4](https://github.com/eholk/ebg/compare/v0.5.3...v0.5.4) - 2025-06-12

### Other

- Fix Docker workflow to tag all semantic versions for manual triggers

## [0.5.3](https://github.com/eholk/ebg/compare/v0.5.2...v0.5.3) - 2025-05-07

### Added

- add category index pages

### Other

- Bump url from 2.5.2 to 2.5.4
- Bump thiserror from 2.0.11 to 2.0.12
- Bump futures from 0.3.30 to 0.3.31
- Bump clap from 4.5.27 to 4.5.37
- Update doc/templates/category.html

## [0.5.2](https://github.com/eholk/ebg/compare/v0.5.1...v0.5.2) - 2025-04-27

### Other

- pin rust-toolchain to 1.86.0
- vibe coding: update dockerfile updates to use release-plz branch to get ebg version
- More vibe coding to improve GitHub actions
- More vibe coding to improve GitHub actions
- Update .github/workflows/release-plz.yml
- Have release-plz update the Dockerfile, and also fix the docker release yml
- Add a Copilot-generated action to automate the docker release
- Fix test and remove unnecessary code
- Update src/generator.rs
- Add basic support for categories
