# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.2](https://github.com/eholk/ebg/compare/v0.6.1...v0.6.2) - 2025-12-26

### Other

- Apply code review suggestion
- Do a better job of automatically updating Dockerfile
- *(deps)* bump tera from 1.20.0 to 1.20.1
- *(deps)* bump tracing-subscriber from 0.3.20 to 0.3.22
- *(deps)* bump syntect from 5.2.0 to 5.3.0
- *(deps)* bump open from 5.3.2 to 5.3.3
- *(deps)* bump clap from 4.5.51 to 4.5.53
- Fix test failure by using absolute path in serve tests
- Address code review comments: use async I/O and improve documentation
- Add support for directory-based posts with --dir flag

## [0.6.1](https://github.com/eholk/ebg/compare/v0.6.0...v0.6.1) - 2025-11-22

### Added

- Add an option to set the post date for a new post

### Other

- Apply code review suggestions around setting default date
- Fix warning
- Bump clap from 4.5.48 to 4.5.51
- Bump serde_json from 1.0.143 to 1.0.145
- Bump tokio from 1.47.1 to 1.48.0
- Bump quick-xml from 0.37.5 to 0.38.3
- Bump indicatif from 0.18.0 to 0.18.2
- Add actions: write permission to fix Docker workflow dispatch

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
