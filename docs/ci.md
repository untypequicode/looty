# CI speed notes

What the workflows cache, why, and how to bust it. Measured on runs of 2026-11-25 (see the PR that introduced this file for the before/after table).

## Where the time goes

| Workflow | Slowest part | Before |
| --- | --- | --- |
| CI | `Browser tests`: 150 s of setup, then 35 e2e files one after the other (~430 s) | ~590 s |
| Self-host | `Caddy and Traefik stacks`: backend image compiled from scratch (~230 s) | ~580 s |
| Docker (release) | same backend image, amd64 and arm64 | 360-460 s |
| Desktop | Windows: `cargo install tauri-cli` compiled from source on a cold cache (~620 s) | ~1020 s |
| Packaging | Flatpak: runtimes + build, no cache | ~410 s |

## What changed

- **Browser tests are sharded** over 3 runners (`E2E_SHARD=i/3`, file index modulo 3, see `e2e/run.mjs`). Each shard starts its own stack and database. No test was removed: the three `N/M test files passed` lines add up to the old total (35). The required check `Browser tests` is now a summary job (`e2e-result`) that needs all shards; the shards are named `Browser tests (shard i/3)`. It is skipped (counts as passed) when the server did not change, as before.
- **Rust cache is written by main only** (`Swatinem/rust-cache` with `save-if`). Pull requests restore from main's cache (key: job + `Cargo.lock` hash + toolchain). Before, each PR that changed anything saved another ~350 MB copy, and the repository was over its 10 GB cache limit, so GitHub kept evicting the useful ones (main's Rust, Docker layers).
- **`backend/Dockerfile` builds dependencies in their own layer** (dummy `main.rs`, then the real sources). The layer is reused from the Docker cache while `Cargo.toml`/`Cargo.lock` are unchanged, so a commit recompiles only the server crate. Same binary: same lockfile, `--locked`, same flags.
- **Self-host image builds read the release cache** (`scope=backend-amd64`, same compile stage as the published image) as well as their own, and only main writes `self-host-backend`.
- **Windows installer: Tauri CLI downloaded prebuilt** from the official `tauri-apps/tauri` release instead of compiled; falls back to `cargo install` if the download fails. It is still cached under `tauri-cli-windows-2`.

## Cache keys and how to bust them

| Cache | Key | Bust by |
| --- | --- | --- |
| Rust (CI) | `v0-rust-<job>-<os>-<hash of Cargo.lock/toolchain>` | change `Cargo.lock`, or `gh cache delete <key>` |
| npm | `node-cache-<os>-npm-<hash of lockfiles>` | change a lockfile |
| Docker layers | gha scopes `backend-amd64`, `self-host-backend`, `frontend-amd64`, `deb-builder`... | `gh cache delete` the `buildkit-blob-*` / `index-*` entries, or change the Dockerfile |
| Debian build | `deb-<hash of Cargo.lock + Dockerfile.deb>` | change either file |
| Windows | `windows-<hash of desktop/src-tauri/Cargo.lock>`, `tauri-cli-windows-2` | bump the `-2` |

Caches saved on a pull request are only readable by that pull request; only caches saved on `main` help the others. Check usage with `gh api repos/untypequicode/looty/actions/cache/usage` (limit 10 GB).

## Not done (options)

- Flatpak (410 s): cache the flatpak-builder state and runtimes (~1.5 GB, would compete for the 10 GB cache limit).
- Windows `target/` cache is only ever saved on PR refs, since Desktop does not run on push to main: add a `push` trigger on `desktop/**` to warm it, at the cost of a main run.
- Build the backend once and share the binary between `Backend tests`/`Browser tests` shards (artifact): saves ~40 s per shard.
- 4 shards instead of 3 (~35 s more).
