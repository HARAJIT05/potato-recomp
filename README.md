# potato-recomp

`potato-recomp` is a low-VRAM, No-Ray-Tracing compatibility and performance fork of the Xbox Series -> Windows Direct3D 12 translation runtime (`VIctory`). It combines upstream high-throughput renderer work (`Belobug`, `cool_fox`, `avatarmanz`, `kelvaroq`, `uiliannn`, `fxceless`, `DexNom`) with 4 GB VRAM / 16 GB RAM residency safeguards so modern AAA console builds remain stable on **4 GB discrete GPUs** and **shared-memory iGPUs / APUs** under `--console series-s`.

> **Legal / Repository Policy Notice**
>
> This repository contains **only** clean-room compatibility runtime source code, build configuration, documentation, and the existing shader helper table (`crates/victory-formats/src/bcpack-tables.bin`). No copyrighted AAA game binaries, dumps, keys, shaders, or assets are included or accepted. All paths and scripts expect you to supply your own legally obtained console build externally (`VICTORY_GAME_DIR`).
> Run `cargo xtask policy` before committing to verify repository hygiene.

---

## Performance Reality Check & Upstream Context

Reported `60 fps` tags on community tracking dashboards belong to lighter arcade/sandbox titles, not the heavyweight open-world AAA flagship. Upstream public Series S open-world throughput tops out around **~22 FPS** on high-end AMD desktop GPUs (`RX 7900 XTX` / `9070 XT` via `Belobug/VIctory-Belobug-AMD` with `7800X3D`-class CPUs). Achieving `10–15+ FPS` cleanly on 4 GB laptops (`RTX 3050 4 GB` / `16 GB RAM`) requires both eliminating post-prison memory/descriptor exhaustion and stacking all CPU submission ports at 720p.

---

## Key Improvements & Stability Fixes (`4 GB` / `16 GB RAM` / Post-Prison Focus)

1. **Out-of-Memory Recovery & 16 GB System-RAM Guard (`host.rs`, `residency.rs`)**
   - Added `Host::reclaim_on_oom()` / `Host::with_oom_retry()` around `read_back_buffer`, `upload_buffer`, `shared_buffer`, `mapped_card_buffer`, and `uav_buffer` allocations. On `E_OUTOFMEMORY` (`0x8007000E`), it trims spare scratch chunks, triggers emergency pool reclamation, releases idle shadow/texture/resource allocations, notifies `residency::allocation_error()`, and retries once instead of cascading into thousands of failed allocations.
   - `VICTORY_RESIDENCY` (`residency_enabled`) defaults **on** for `<= 32 GiB` hosts (disable with `VICTORY_RESIDENCY=0`), and `AVAILABLE_PHYS` (`has_physical_growth_reserve`) prevents staging/prefetch growth from thrashing the pagefile on `16 GB` systems.
2. **Dynamic Descriptor Slot Headroom & Full Port Stack (`descriptor/host.rs`, `descriptor_sites.rs`, `port*.rs`, `plaincopy.rs`, `pipeline.rs`)**
   - Updated `dynamic_slots_on()` so `VICTORY_DYNAMIC_SLOTS` (e.g., `327680`) is honored up to `(null / 2).max(DYNAMIC)` on NVIDIA's 1M-view capped heap (`PORTABLE_VIEWS = 1_000_000`) instead of hard-clamping to `262,144` and stalling post-prison world frames with `no free dynamic descriptor slots`.
   - Integrated `avatarmanz`'s additional descriptor-table copy hooks (`COPY_CALLS` + `COPY_128`, `VICTORY_PORT_COPIES=1`) alongside `Belobug` `PR #354`, plus Renderer Ports 1, 2, 5, 6, and 7:
     - `VICTORY_PORT_CONSTANTS=1` + `VICTORY_PORT_CONSTANTS_ORDER=1` (32-bit console graphics heap in place via `OpenExistingHeapFromAddress1`)
     - `VICTORY_PORT_DESCRIPTORS=1` + `VICTORY_PORT_COPIES=1` (title descriptor-record stores & table `memcpy`/AVX copies track dirty slots directly)
     - `VICTORY_PORT_CBV=1` (reads aligned table/root constant buffers in place as raw views)
     - `VICTORY_PORT_GUARD=1` (shared/per-block write-watch guard lock)
     - `VICTORY_PORT_JOURNAL=1` (per-partition watch journal lock + overlap heap sync)
   - Added `VICTORY_COPY_PLAIN=1` (`plaincopy.rs` `rep movsb` + guarded read path), skipped unused table UAV writeback (`TablePlan`), and added opt-in `VICTORY_SHADER_FAST_BITS=24` (`bitcode.rs` `nsz + arcp`).
3. **720p Series-S Output Without FSR4 Mismatch (`Play-Potato-4GB.cmd`, `resolution.rs`)**
   - Forces `VICTORY_OUTPUT=1280x720` together with `VICTORY_UPSCALER=6` (the title's own upscaler instead of FidelityFX `7`), avoiding the Series S `Shipping FSR4 configuration mismatch: maxUpscaleSize must be 1920x1080; received 1280x720` error while cutting fillrate and render-target pressure.
   - Sets `VICTORY_FRAME_INTERVAL_MS=16.667` and `VICTORY_TITLE_PRIORITIES=0` so title worker threads do not starve D3D12 submission threads.
4. **No-RT Indoor Lighting & Accurate Diagnostics (`ambient.rs`, `pipeline/make.rs`)**
   - Sets `DEFAULT_FLOOR = 0.025` (`VICTORY_AMBIENT_MIN`, `0`/`off` to disable), caps volumetric indoor haze in `clear_air()` (`VICTORY_AMBIENT=0.8`, `VICTORY_AMBIENT_AIR=1.5`), and enables clock-based ambient adaptation (`VICTORY_AMBIENT_SETTLE=250,3`) so interiors and loading transitions remain legible without DXR (`VICTORY_RT_RESOLVE=0`, `VICTORY_ACTIVATE_RAYS=0`).
   - Restored `missing()` reporting `p.left_out` for intentionally skipped RT/hit pipelines.
5. **Low-VRAM Budgeting, Aliasing Crash Fix, & Wave/Wait Stability (`host.rs`, `resource.rs`, `pool.rs`, `texture.rs`, `fence.rs`, `victory-shader`, `victory-hle`)**
   - Tracks `POOLED` resources so `record_pool_aliasing_barrier` skips committed non-pooled textures (`VICTORY_TEXTURE_POOL=0` no longer crashes `CommandList::Close()` with `0x80070057`), and uses `LOW_VRAM_BLOCK` (`4 MiB`) / `LOW_MEM_BLOCK` (`16 MiB`) pools on constrained hardware.
   - Cached `Host::dedicated_video_memory()` and `resource::room_used_limit_bytes()` allow discrete 4 GB GPUs to use up to `VICTORY_TEXTURE_ROOM_PERCENT` (`90%` default in `Play-Potato-4GB.cmd`) of dedicated VRAM while evicting idle allocations against DXGI's budget.
   - Bounded `WaitOnAddress` (`250 ms` slices releasing held fence queues), enabled `VICTORY_XMA_REFILL` by default (`VICTORY_XMA_REFILL=0` to disable), restored Wave64->Wave32 lane-mask/guard fixes (`victory-shader`), and added optional PC mouse/keyboard controls (`VICTORY_KBM_PC=1`).

---

## Quick Start

### Prerequisites

- **OS:** Windows 10 / 11 x64
- **CPU:** x86-64 with AVX2 support (checked at startup)
- **GPU:** Direct3D 12 compatible discrete GPU (4 GB+ VRAM tested) or APU/iGPU with latest drivers
- **Toolchain:** Rust toolchain matching [`rust-toolchain.toml`](rust-toolchain.toml)
- **External Title Files:** Your own legally obtained console build directory placed outside the repository

### Build

```powershell
cargo build --locked --release -p victory
```

Optional diagnostics binary (adds frame capture/replay instrumentation at significant runtime CPU cost; use `--release` for normal play):

```powershell
cargo build --locked --profile diag -p victory --features diagnostics
```

### Configure & Launch (`4 GB` / No-RT Profile)

Set `VICTORY_GAME_DIR`, optionally pass the executable name as the first script argument (otherwise [`Play-Potato-4GB.cmd`](Play-Potato-4GB.cmd) selects the first `.exe` in `VICTORY_GAME_DIR`), and launch:

```cmd
set "VICTORY_GAME_DIR=C:\Path\To\AAA.Game.Dump"
Play-Potato-4GB.cmd
```

Enable 5-second FPS/draw logging explicitly via `VICTORY_FPS_LOG=1` (already enabled in [`Play-Potato-4GB.cmd`](Play-Potato-4GB.cmd)). By default, cache, saves, and logs are stored under `%LOCALAPPDATA%\VIctory\` (`VICTORY_CACHE_DIR` / `VICTORY_DATA_DIR` override them). See [`victory.example.toml`](victory.example.toml), [`Play-VIctory.cmd`](Play-VIctory.cmd), and [`scripts/play.ps1`](scripts/play.ps1) for upstream profiles.

Useful CLI subcommands (`target\release\victory.exe --help`):

- `victory inspect <exe> [--imports] [--exports]` — inspect PE headers and console imports
- `victory census <paths...>` — check HLE coverage of imported DLLs/symbols
- `victory shaders <game-dir>` — pre-translate shader containers into the local cache
- `victory translate <paths...> [--validate] [--pipelines]` — test shader lowering & DXIL validation
- `victory warm` — pre-compile D3D12 PSOs recorded via `VICTORY_PIPELINE_BOOK=1`
- `victory run <exe> --console series-s --execute` — launch the title under the compatibility layer

---

## Observed Verification & Benchmarks

### Policy, Lints & Build

All checks below ran on Windows with `--locked` (`CARGO_TARGET_DIR` set outside the repository):

| Check | Command | Result |
|---|---|---|
| Asset & file allowlist | `cargo run --locked -p xtask -- policy` | Passed (`asset policy: 275 files ok`) |
| Workspace Clippy (default) | `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed (`0` warnings) |
| Workspace Clippy (`--all-features`) | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | Passed (`0` warnings) |
| Release binary | `cargo build --locked --release -p victory` | Passed (`3m 11s`) |

### Workspace Test Suite

| Crate / Target | Passed | Failed | Ignored | Time | Notes |
|---|---:|---:|---:|---|---|
| `victory` (bin + 4 integration targets) | 32 | 0 | 0 | `5.10s` | `25 + 2 + 3 + 1 + 1` passed |
| `victory-core` | 13 | 0 | 0 | `0.01s` | Config & repo-boundary policy tests |
| `victory-formats` | 41 | 0 | 5 | `12.12s` | Texture swizzling, BCPack, XMA |
| `victory-game` | 3 | 0 | 0 | `0.00s` | `sgd5` container tests |
| `victory-gpu` (`--lib`) | 408 | 0 | 34 | `39.37s` | Includes `low_vram_room_tests`, `dynamic_slots_honor_override_on_capped_and_uncapped_heaps`, `aliasing_barrier_skips_committed_textures`, `residency_defaults_on_for_limited_ram_and_respects_override`, `a_store_marks_only_the_table_descriptor_it_names`, `the_128_byte_copy_stores_and_marks_its_four_slots`, and both `indirect` tests |
| `victory-hle` (`--lib` + 2 integration targets) | 165 | 0 | 2 | `3.57s` | `162 + 1` isolated + `1` `fidelityfx` + `1` `no_dll_planting` |
| `victory-loader` (`--lib` + 2 integration targets) | 60 | 0 | 0 | `1.71s` | `57 + 2 + 1` passed |
| `victory-pe` | 36 | 0 | 0 | `0.01s` | PE32+ and import library parser |
| `victory-pipeline` | 7 | 0 | 0 | `0.10s` | Content-addressed converter cache |
| `victory-shader` | 304 | 0 | 4 | `347.03s` | DXIL emission, VALU, waterfall, Wave64->Wave32 tests |
| `xtask` | 9 | 0 | 0 | — | Policy enforcement and documented switch consistency |

### Local Test Hardware & Observations

**Test Hardware:** `AMD Ryzen 7 5800H`, `NVIDIA GeForce RTX 3050 Laptop GPU` (`4,096 MiB` physical VRAM / `3,965 MiB` reported by D3D12, `3,370 MB` DXGI local budget, `7,212 MB` shared memory), `16 GB` system RAM, Windows 11, `--console series-s`, No-RT (`VICTORY_RT_RESOLVE=0`, `VICTORY_ACTIVATE_RAYS=0`).

1. **`--release` Startup & Shader Warmup Smoke Run (`150.3s`, `VICTORY_OUTPUT=1280x720`, `VICTORY_UPSCALER=6`):**
   - Verified Renderer Port 1 (`193,986,560` bytes / `182,452,224` bytes in place), Renderer Port 2 (`6/6` descriptor stores + `3/3` `COPY_CALLS`/`COPY_128` hooks), `VICTORY_DYNAMIC_SLOTS=327680` (`title_in_place=672319`), and cache warmup (`4,088` cache files, `435.1 MB`, `193.7 MB` host pipeline library).
   - Presented `8,192` title/menu frames (`frame=1024 -> 8192` paced at exact `60.0 fps` at `5.5` draws/frame) in a headless agent session (`DXGI_STATUS_OCCLUDED` `0x087A0001` reported once; background session could not focus the HWND to click Continue into 3D world gameplay).
   - Logged **`0` crashes, `0` `0x8007000E` OOM errors, `0` `no free dynamic descriptor slots` errors, `0` `port_descriptors` fallback misses, and `0` `FSR_API_DEBUG_ERROR` mismatches** (compared against `84,766` `0x8007000E` failures, `17,486` fallback misses, and `1` dynamic descriptor exhaustion failure in the pre-patch log).
   - Play [`Play-Potato-4GB.cmd`](Play-Potato-4GB.cmd) in an interactive Windows desktop session to measure real 3D open-world FPS via `VICTORY_FPS_LOG=1` (`benchmark frame timing`).
2. **Baseline `--profile diag --features diagnostics` Captures (`250–270 s` instrumented runs with frame PNG dumps):**

| Run | Key Configuration | FPS / Frame Time (`diag`) | Host Texture Copies Created | Blocked Passes | `tiles` Heaps | Peak/Steady Card & Shared | Stability & Visual Outcome |
|---|---|---|---|---:|---:|---|---|
| `cap_fixed` | `POOL=4M`, `ROOM=85%`, `CARD_BUFFERS=288`, `MIN_KB=512`, `TARGETS=0` | `1.8 fps`, `569.0 ms/f` (`226.0` prep, `302.4` GPU, `112.3` back), `4,584` draws/f | `23,678` without copy | `1,119` | — | `4,177 MB` card, `5,391 MB` shared | `1,748 MB` pinned in `pool::TEXTURES` for `996 MB` live (`752 MB` dead holes); black characters/rooms |
| `cap_pool0` | `POOL=0`, `ROOM=95%`, `CARD_BUFFERS=288`, `MIN_KB=512`, `TARGETS=0` | `1.2 -> 0.9 fps`, `1074.9 ms/f` (`205.9` prep, `727.3` GPU, `148.9` back), `4,624` draws/f | `21,027` created (`10,550` alive, `1,249 MB`) | `408` (`11,889` missing) | — | `3,743 MB` card, `5,341 MB` shared | Crisp indoor lighting & exteriors; main character sharp, secondary character head still black |
| `cap_tuned` | `POOL=0`, `MIN_KB=128`, `CARD_BUFFERS=64`, `BUDGET=4000`, `ROOM=95%` | `0.3 fps`, `3582.7 ms/f`, `7,128` draws/f | `32,311` created (`22,106` due slots) | `0` | `832 MB` (`12,313` made) | `3,807 MB` card, `4,681 MB` shared | **Regression:** `MIN_KB=128` caused `heap buffers` spikes of `442–3,703 ms` |
| `cap_clean` | `POOL=0`, `MIN_KB=512`, `CARD_BUFFERS=288`, `BUDGET=4050`, `ROOM=95%` | `119` PNGs up to `frame-1730` | `27,049` created | `0` | — | `4,684 MB` (peak `4,978 MB`), `5,760 MB` shared | All visible faces crisp, **crashed at `227s` (`frame-1735`)** with `0x887A0005` (`DXGI_ERROR_DEVICE_REMOVED`) |
| `cap_sweet` | `POOL=0`, `MIN_KB=512`, `CARD_BUFFERS=128`, `BUDGET=1750`, `ROOM=95%` | `1.6 fps`, `613.7 ms/f` (`262.1` prep, `282.9` GPU, `172.2` back), `4,583` draws/f | `25,991` created | `353` | `320 MB` | `3,748 MB` card, `4,962 MB` shared | Stable run with modest missing-texture pressure |
| `cap_gold` | `POOL=0`, `MIN_KB=512`, `CARD_BUFFERS=96`, `BUDGET=1950`, `ROOM=85%` | `1.0 fps`, `1042.9 ms/f` (`130.9` GPU) | `24,294` created (`5` copies `80 MB` card buffers) | `256` | — | `3,692 MB` card, `4,963 MB` shared | Stable run, low GPU ms, manual budget still slightly conservative |
| `cap_verified` | Auto `room_used_limit_bytes`, `TARGETS=1024`, `CARD_BUFFERS=96`, `ROOM=95%`, `POOL=0`, `MIN_KB=512` | `1.3 fps`, `773.7 ms/f` (`486.1` prep, `248.9` GPU, `304.8` back), `3,640` draws/f | `32,692` created (`4` copies `80 MB` card buffers) | `394` | `448 MB` | `3,769 MB` card, `4,740 MB` shared | `198` PNGs (`frame-2915`, `270.5s`, **0 crashes**) |
| `cap_verified2` | Same as above + `TARGETS_MB=1280` | `1.1 fps`, `897.8 ms/f` (`316.4` prep, `211.6` GPU, `241.7` back), `3,648` draws/f | **`34,056` created** | `283` | `320 MB` (`1` heap freed) | **`3,647 MB` card, `4,570 MB` shared** | `175` PNGs (`frame-2576`, `250.2s`, **0 crashes**), lowest shared footprint |

---

## Future Developer Guide: Pushing Low-End Hardware Further

1. **Eliminate Remaining Descriptor Fallback Sites (`descriptor_sites.rs`)**: Post-prison open-world frames occasionally hit unhooked writer PCs (`+0x18849c`, `+0x4e9b4e2`, `+0x2474afc`). Disassembling and adding those copy/store patterns removes fallback descriptor lookups altogether.
2. **Sub-`64 MiB` `tiles` Heap Packing (`tiles.rs`)**: `tiles` allocates `64 MiB` placed heaps, which restricts eviction when one resident mip tail pins an entire heap. Mirroring `pool.rs`'s `4 MiB` (`LOW_VRAM_BLOCK`) / `16 MiB` strategy or segregating persistent mip tails vs. transient mips would reclaim hundreds of megabytes on 4 GB cards.
3. **Render-Target Aliasing (`resource.rs`, `host.rs`)**: Series S render targets reserve ~`1.3 GiB` (`VICTORY_CARD_ROOM_FOR_TARGETS_MB=1280`). Aliasing transient G-buffer/shadow render targets with non-overlapping lifetimes frees substantial VRAM on 4 GB cards.
4. **Complete Wave64 -> Wave32 Compute Lowering (`victory-shader/src/emit/`)**: Remaining dropped compute hashes (`369cbe906ea72764`, `02384a60fda6f2d0`, `6b82bc7d22016cb5`, `5dc3b25986622949`, `6f225b641c5b0708`) fail Wave32 lowering under the safe path; finishing symbolic Wave64 reduction/lane-mask lowering avoids dropped compute dispatches cleanly without `VICTORY_WAVE_WIDE` TDR risk.
5. **Test Pool vs. Committed Mips & Pre-Warm Caches**: Benchmark `VICTORY_TEXTURE_POOL=0` vs `1` (4 MiB blocks) on open-world saves, and pre-warm with `victory shaders <game-dir>` + `VICTORY_PIPELINE_BOOK=1` + `victory warm`.

See also [`docs/findings.md`](docs/findings.md), [`docs/architecture.md`](docs/architecture.md), [`docs/fork-ray-tracing.md`](docs/fork-ray-tracing.md), [`docs/bug-journal.md`](docs/bug-journal.md), and [`docs/replay.md`](docs/replay.md).

---

## Credits & Upstream Acknowledgments

`potato-recomp` builds directly on the open-source research and compatibility-layer engineering of the following repositories and contributors (GPL-3.0-or-later; see [`LICENSE`](LICENSE)):

- **Upstream Foundation:** [`Marvisak/VIctory`](https://github.com/Marvisak/VIctory) — core loader, PE/format decoders, HLE modules, D3D12 runtime, and shader recompiler
- **High-Throughput Sync & No-RT Base:** [`Belobug/VIctory-Belobug-AMD`](https://github.com/Belobug/VIctory-Belobug-AMD) (`beloooo`, `romainhedouin`) — Renderer Ports 1/5/6/7, ambient settling, parallel prelude uploads, buffer sweep, `RetailPlane` compose, and pipeline workers
- **Windows Pareto-Residency Base:** [`codec-d/cool_fox`](https://github.com/codec-d/cool_fox) — Windows residency framework, Wave32 lowering, `port_descriptors`, two-plane compose, and DXIL fixes
- **Descriptor Table Hooks & Telemetry:** [`avatarmanz/VIctory-7900XTX`](https://github.com/avatarmanz/VIctory-7900XTX) — `COPY_CALLS` / `COPY_128` descriptor copy hooks, `VICTORY_THIN`, `VICTORY_ASYNC_SUBMIT`, `VICTORY_JOIN_PARTS`, `VICTORY_RELAY`, `VICTORY_CARD_STEPS`, and `docs/findings.md`
- **Community Forks & Feature Contributors:**
  - `kelvaroq` — `plaincopy` (`VICTORY_COPY_PLAIN=1`) fast guarded memory copies
  - `uiliannn` ([`uiliannn/VItoriaBr`](https://github.com/uiliannn/VItoriaBr)) — capped-heap `VICTORY_DYNAMIC_SLOTS` override and unused table UAV writeback skipping
  - [`fxceless/victory-fork`](https://github.com/fxceless/victory-fork) (`fxceless`) — texture readback, small card buffers, buffer-byte hashing, fast-math bitflags (`VICTORY_SHADER_FAST_BITS`), [`docs/README-fxceless.md`](docs/README-fxceless.md)
  - [`DexNom/VIctory`](https://github.com/DexNom/VIctory) — ray-dispatch thread counts, cube gradients, sparse range coalescing
  - [`FQY55kai/ZJOU-VICTORY`](https://github.com/FQY55kai/ZJOU-VICTORY) — render-target binding & shader fixes
  - [`myokex/victory-amd`](https://github.com/myokex/victory-amd) (`Spexivus`, `joroyt`, `tightfall`, `Joshua Odom`)
  - Individual community contributors credited in source history: `Marvis`, `Duff`, `codec_d`, `fxceless`, `Geckomacabre`, `cxy0072019-sketch`, and all contributors recorded in upstream commit logs.
- **Reference Layers & Tooling Studied Upstream:**
  - [`ivall/xcl`](https://github.com/ivall/xcl) (`GPL-2.0-only` console instruction/runtime reference)
  - [`ZSlayerHQ/victory-world-runtime`](https://github.com/ZSlayerHQ/victory-world-runtime), [`Zettrox/ztrx`](https://github.com/Zettrox/ztrx), [`R0Y00/Xbox2Windows`](https://github.com/R0Y00/Xbox2Windows)
  - [Microsoft DirectX Shader Compiler (`DXC`)](https://github.com/microsoft/DirectXShaderCompiler)
