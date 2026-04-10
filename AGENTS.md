# AGENTS.md

## Must-follow constraints

- All routes are GET-only. The CORS layer explicitly allows only `GET`. Do not add mutating endpoints without updating the CORS config in `main.rs`.
- All routes must be nested under `/api` — the router in `main.rs` wraps everything with `.nest("/api", ...)`.
- `/api/health` is exempt from rate limiting by hardcoded path check in `middleware.rs`. Do not rename it.
- The cubiomes C library (`cubiomes/`) is a git submodule. Do not modify any files inside it. All C interop goes through `cubiomes_shim.c` and `src/ffi/mod.rs`.
- All cubiomes FFI calls are blocking and CPU-intensive. They must run inside `tokio::task::spawn_blocking`. Never call `BiomeGenerator` methods directly in an async context.
- `BiomeGenerator` is `Send` but not `Sync`. Do not share it across threads; construct a new one per `spawn_blocking` closure.
- Seed strings that fail `i64::parse` are hashed via `java_string_hash` (Java-compatible `s * 31` hash). This is intentional — do not replace with Rust's default hashing.
- `version_to_mc_const` in `services/mojang.rs` is the single source of truth for supported MC versions. Adding a new version requires adding a constant in `src/ffi/mod.rs` matching the cubiomes `MC_*` integer value, then mapping it here.

## Validation before finishing

```bash
cargo build          # must succeed — also compiles cubiomes via cc crate
cargo clippy -- -D warnings
```

A C compiler must be present. The `cc` crate compiles cubiomes at build time via `build.rs`. Build failures here are usually missing headers or a bad `cubiomes/` submodule state.

## Repo-specific conventions

- Rate limiters are defined in `Limiters::new()` in `middleware.rs`. Route-specific limits are matched by path prefix (`/api/seedmap`, `/api/serverjars`). New routes with different limits must add a new `RateLimiter` field there and a matching branch in `rate_limit_middleware`.
- Caching uses `moka::future::Cache` with `try_get_with`. TTLs are set per-router in the `router()` fn of each route file, not globally.
- Each route module owns its own `AppState` struct and `router()` fn. State is not shared between route modules.
- `reqwest::Client` is constructed once in `main.rs` with a 10s timeout and shared via clone into each router.
- `ALLOWED_ORIGINS` env var controls CORS. Comma-separated. Defaults to `https://spoak.cc,http://localhost:3000`.
- `PORT` env var controls listen port. Defaults to `4000`.

## Important locations

- `src/ffi/mod.rs` — all cubiomes FFI declarations, `BiomeGenerator` wrapper, and MC version / structure type constants
- `cubiomes_shim.c` — only C file to edit; provides `cubiomes_alloc_generator` / `cubiomes_free_generator` since cubiomes `Generator` is opaque
- `build.rs` — lists every cubiomes `.c` file that must be compiled; update here when adding new cubiomes source files
- `src/middleware.rs` — all rate limiter definitions and IP extraction logic (trusts `X-Forwarded-For` only from private/loopback IPs)

## Known gotchas

- `allocCache` from cubiomes returns a raw pointer that must be freed with `free()` (not Rust's allocator). The `get_biomes` method in `BiomeGenerator` does this correctly — do not refactor it to use `Vec::from_raw_parts` without matching the allocator.
- Tile biome data is returned as `i16` little-endian bytes (`application/octet-stream`), not JSON. The frontend expects this binary format.
- `isViableStructurePos` mutates generator internal state. `find_structures` takes `&mut self` for this reason — do not change it to `&self`.
- Player profile cache key is lowercased username; Mojang API lookup uses the original-case username. Both are intentional.
- `PaperDownloads` has two optional fields (`server:default`, `application`) because the Paper API changed field names across versions. Both must remain.
