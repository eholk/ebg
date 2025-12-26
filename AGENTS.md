# Development Notes for AI Agents

This file contains context and notes for AI agents working on the EBG (Eric's Blog Generator) project.

## Project Overview

EBG is a static site generator written in Rust. It processes markdown posts and generates a static website.

## Current Feature: Wayback Machine Integration

We're implementing a feature to automatically archive external links using the Internet Archive's Wayback Machine. This helps preserve linked content even if the original pages disappear.

### Design Decisions

**Per-Post Configuration**: Each post has its own `.wayback.toml` file tracking archived links, rather than a site-wide database.

**Rationale**:
- Modularity - links travel with their posts
- Version control clarity - see what links were archived per post
- No merge conflicts across posts
- Temporal accuracy - different posts archive at different times
- Clean deletion - delete post, delete its wayback data

**File Locations**:
- Single-file post: `_posts/2023-01-25-hello.md` → `_posts/2023-01-25-hello.wayback.toml`
- Directory post: `_posts/2023-01-25-hello/index.md` → `_posts/2023-01-25-hello/wayback.toml`

### Architecture

**Module Organization**:
- `src/wayback/` - Pure Wayback Machine API client (could be extracted as standalone crate)
- `src/index/wayback_links.rs` - Per-post wayback config data structures
- `src/index/links.rs` - Link parsing and classification (shared between renderer and wayback)
- `src/index/page.rs` - Added `external_links()` method to `PageSource`
- `src/cli/wayback.rs` - CLI command implementation

**Key Types**:
- `WaybackLinks` - Collection of archived links for a post (serializes to TOML)
- `WaybackLink` - Single archived link (url, wayback_url, archived_at)
- `LinkDest` - Parses and classifies link destinations (External/Local/Email)
- `WaybackConfig` - Site-wide configuration with filters

### Filtering System

Posts can be filtered from wayback archiving via `Site.toml`:

```toml
[wayback]
snapshots = "wayback.toml"
exclude = [{ before = "2025-12-01" }]
```

The `WaybackConfig::should_exclude_post()` method checks if a post matches exclusion criteria.

## Implementation Progress

### ✅ Phase 1: Data Structures & File I/O (COMPLETE)
- Created `WaybackLinks` and `WaybackLink` structures
- TOML serialization/deserialization
- File I/O methods (`from_file`, `to_file`)
- Query methods (`find`, `contains`, `iter`)
- Full test coverage (10 tests)

### ✅ Phase 2: Link Extraction (COMPLETE)
- Refactored `LinkDest` from renderer to `src/index/links.rs` (shared module)
- Added `PageSource::external_links()` method
- Uses `pulldown-cmark` to parse markdown
- Filters for http/https external URLs only
- Test coverage for link extraction

### ✅ Phase 3: CLI Command Skeleton (COMPLETE)
- Implemented `ebg wayback update-links [ROOT]` command
- Scans all posts for external links
- Determines wayback config file paths
- Loads existing wayback configs
- Reports what needs archiving vs already archived
- Dry-run mode (doesn't actually archive yet)

### ✅ Phase 3.5: Filtering Support (COMPLETE)
- Reads wayback config from `Site.toml`
- Implements `WaybackFilter::Before(date)` exclusion
- Shows active filters in command output
- Tracks and reports filtered post count

### 🔜 Phase 4: API Integration (NEXT)
- Get Wayback credentials from environment variables
- For each link needing archiving:
  - Call `wayback.begin_save_page()`
  - Poll `wayback.job_status()` until complete
  - Extract wayback URL from successful response
  - Add to `WaybackLinks` and save to `.wayback.toml`
- Handle rate limiting and errors gracefully
- Progress indicators for long-running operations

### 🔜 Phase 5: Rendering Integration (FUTURE)
- Modify HTML renderer to add archive indicators
- Add little icon or "(archived)" link next to external links
- Make indicator style configurable

## Testing

**Test Philosophy**:
- Unit tests for data structures and parsing
- Integration via CLI command on real blog
- Test blog has 75 posts, 355 external links
- Current filter excludes posts before 2025-12-01 (leaves 1 post)

**Running Tests**:
```bash
cargo test --lib                    # Run all library tests
cargo build --bin ebg               # Build CLI
target/debug/ebg wayback update-links ../blog  # Test on real blog
```

## Key Files

- `ebg/src/index/wayback_links.rs` - Per-post wayback config data structures
- `ebg/src/index/links.rs` - Link classification (LinkDest)
- `ebg/src/index/page.rs` - PageSource with external_links() method
- `ebg/src/wayback/api.rs` - Wayback Machine API client
- `ebg/src/cli/wayback.rs` - CLI command implementation
- `ebg/src/index.rs` - WaybackConfig and filtering logic

## Important Notes

### Existing Code to Preserve
- `src/wayback.rs` contains old `Snapshot` structure (site-wide format)
- It's leftover from an earlier design; can be cleaned up later
- Don't confuse it with the new per-post approach

### API Credentials
The Wayback API requires credentials:
- Access key: `WAYBACK_ACCESS_KEY` environment variable
- Secret key: `WAYBACK_SECRET_KEY` environment variable
- Obtain from: https://archive.org/account/s3.php

### API Rate Limits
- Be respectful of Wayback Machine API limits
- Add delays between requests if needed
- Handle "already archived recently" responses gracefully

### Error Handling
- Use `miette` for error types (derive `Diagnostic`)
- Provide helpful error messages with context
- Don't panic on API errors - log and continue

## Code Style

- Use `clap` for CLI argument parsing
- Async code uses tokio runtime
- Error handling: `miette::Result<T>`
- Tests use `tempfile::TempDir` for temporary directories
- Serialize/deserialize with `serde` and `toml`

## Dependencies Already Added

- `pulldown-cmark` - Markdown parsing
- `url` - URL parsing and manipulation
- `chrono` - Date/time handling
- `serde`, `toml` - Serialization
- `miette`, `thiserror` - Error handling
- `clap` - CLI parsing
- `tempfile` - Test utilities (dev dependency)

## Next Session Start Here

Phase 4 implementation: Integrate the Wayback Machine API to actually archive links and save the results to `.wayback.toml` files.

Key considerations:
1. Read credentials from environment variables
2. Handle job polling - API is async, jobs take seconds to complete
3. Parse the timestamp from successful responses to build wayback URLs
4. Save updated WaybackLinks to file after each successful archive
5. Handle errors gracefully - don't fail entire run if one link fails
6. Consider batching or rate limiting for many links