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

## Key Improvements & Stability Fixes (`4 GB` / `16 GB RAM` / Character, Interior & Speed Stack)

1. **Out-of-Memory Recovery, Live-Budget Residency & 16 GB RAM Protection (`host.rs`, `residency.rs`, `resource.rs`, `tiles.rs`, `pool.rs`)**
   - Added `Host::reclaim_on_oom()` / `Host::with_oom_retry()` around `read_back_buffer`, `upload_buffer`, `shared_buffer`, `mapped_card_buffer`, and `uav_buffer` allocations. On `E_OUTOFMEMORY` (`0x8007000E`), it trims spare scratch chunks, triggers emergency pool reclamation, releases idle shadow/texture/resource allocations, notifies `residency::allocation_error()`, and retries once instead of cascading into failed allocations.
   - `VICTORY_RESIDENCY` (`residency_enabled`) defaults **on** for `<= 32 GiB` hosts (disable with `VICTORY_RESIDENCY=0`).
   - Fixed 4 GB admission & eviction (`room_admits()`, `texture_eviction_policy()`, `targets_room_kept()`, `pool::room()`): ignores DXGI non-local (`shared`) usage because WDDM includes `UPLOAD`/`CUSTOM` system-RAM heaps (`~5+ GiB` shadow/staging buffers), drops `targets_room_kept()`'s default reservation on `<= 6 GiB` discrete GPUs from `3,072 MiB` to `1,350 MiB` (`VICTORY_CARD_ROOM_FOR_TARGETS_MB`), trims only on local VRAM saturation (`>= 90%`) or critical host headroom (`< 512 MiB` free RAM / critical commit), and keeps `auto_texture_max` at `512` up to `85%` of budget before stepping down to `256`.
   - Supports `4 MiB` texture pool blocks (`LOW_VRAM_BLOCK`) and shrunk `8 MiB` placed tile heaps (`LOW_VRAM_HEAP_TILES = 128`) on constrained VRAM/RAM hosts.
2. **Release-Build RT Work Skip List (`pipeline.rs`, `draw.rs`, `scripts/rt_off_hashes.txt`, `scripts/rt_work_off_hashes.txt`)**
   - Enabled `VICTORY_SKIP_PIPELINES` and added `VICTORY_SKIP_PIPELINES_FILE` (`;`-separated list files, sorted binary search) in standard `--release` builds (no `--features diagnostics` overhead required).
   - Bundles the RT resolve skip list (`scripts/rt_off_hashes.txt`) and RT-only compute work skip list (`scripts/rt_work_off_hashes.txt`: `bvh_skinning` & probe `GatherCopy`), removing GPU work that only serves disabled DXR passes while avoiding `VICTORY_CARD_STEPS=2` card saturation on 4 GB cards.
3. **4 GB Character/Skin Residency, NaN Scrubbing & Interior Veil Fixes (`resource.rs`, `descriptor/host.rs`, `list.rs`, `texture.rs`, `draw.rs`, `ambient.rs`, `lower.rs`, `shader_cache.rs`)**
   - Ported **`VICTORY_TEXTURE_DROP_MIPS`** + **`VICTORY_TEXTURE_KEEP_SIDE`** (`2` mips dropped above `128` px) so ~4x more world, clothing, and character textures fit in 4 GB VRAM without hitting admission walls or losing mip-level copies/views/readbacks.
   - Ported **`VICTORY_BAKE_SOURCES=full`** (plus 7 `BAKE_PIXEL_SHADERS`) and **`VICTORY_ZERO_RELOOK_FRAMES=2`** so skin-bake draws upload source textures ahead of baking and partially mapped all-zero skin textures retry every `2` frames.
   - Ported **`VICTORY_NAN_SCRUB=1`** (`lower.rs`, `shader_cache.rs`) to zero NaN/infinite pixel outputs and `f32` stores before TAA/bloom spread black/white artifacts, plus **`VICTORY_AMBIENT_MARCH=3`**, **`VICTORY_AMBIENT_MOST`**, froxel NaN zeroing in `clear_air()`, and `FILL_VIEWS = 4096` (`ambient.rs`, `draw.rs`) to remove interior haze veils without DXR.
4. **Dynamic Descriptor Slot Headroom & Full Port Stack (`descriptor/host.rs`, `descriptor_sites.rs`, `port*.rs`, `plaincopy.rs`, `xmem.rs`)**
   - Honors `VICTORY_DYNAMIC_SLOTS=327680` on NVIDIA's 1M-view capped heap (`PORTABLE_VIEWS = 1_000_000`) to prevent post-prison world stalls.
   - Enables Renderer Ports 1/2/5/6/7 (`VICTORY_PORT_CONSTANTS`, `VICTORY_PORT_CONSTANTS_ORDER`, `VICTORY_PORT_DESCRIPTORS`, `VICTORY_PORT_COPIES`, `VICTORY_PORT_CBV`, `VICTORY_PORT_GUARD`, `VICTORY_PORT_JOURNAL`), `VICTORY_COPY_PLAIN=1`, `VICTORY_BIND_CACHE` (redundant PSO/RT skip), `VICTORY_MAP_THREADS=8`, and `VICTORY_UPSCALER=6` + `VICTORY_RENDER_SCALE=0.5` (Series S internal half-scale without FSR4 table mismatches).
5. **Telemetry, Wait Stability & PC Input (`host.rs`, `fence.rs`, `victory-shader`, `victory-hle`)**
   - Extended `VICTORY_FPS_LOG=1` with `cpu_ms`, `gpu_ms`, `copy_ms`, `texture_ms`, `back_ms`, `own_mb`, `budget_mb`, `shared_mb`, and `avail_ram_mb` at `WARN` level every 5 seconds.
   - Bounded `WaitOnAddress` (`250 ms` slices), enabled `VICTORY_XMA_REFILL` by default, kept safe Wave64->Wave32 lowering (`VICTORY_SHADER_FAST_BITS` left unset by default), and enabled `VICTORY_KBM_PC=1`.

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

Copy [`victory.example.toml`](victory.example.toml) to git-ignored `victory.toml` and point `[paths]` `game_dir`, `cache_dir`, and `data_dir` at your external folders (for example, `game_dir` on a secondary drive like `E:\` while keeping build/cache/save data on `D:\` or `%LOCALAPPDATA%`), or set `VICTORY_GAME_DIR` in your environment. Optionally pass the executable name as the first argument to [`Play-Potato-4GB.cmd`](Play-Potato-4GB.cmd):

```cmd
set "VICTORY_GAME_DIR=E:\Path\To\AAA.Game.Dump"
Play-Potato-4GB.cmd
```

If `%~dp0victory.toml` is present, [`Play-Potato-4GB.cmd`](Play-Potato-4GB.cmd) reads `game_dir` automatically when `VICTORY_GAME_DIR` is unset and lets `victory.exe` load `cache_dir` / `data_dir` directly from `victory.toml`. See also [`scripts/play.ps1`](scripts/play.ps1) and [`Play-VIctory.cmd`](Play-VIctory.cmd) for upstream profiles.

Useful CLI subcommands (`target\release\victory.exe --help`):

- `victory inspect <exe> [--imports] [--exports]` — inspect PE headers and console imports
- `victory census <paths...>` — check HLE coverage of imported DLLs/symbols
- `victory shaders <game-dir>` — pre-translate shader containers into the local cache
- `victory translate <paths...> [--validate] [--pipelines]` — test shader lowering & DXIL validation
- `victory warm` — pre-compile D3D12 PSOs recorded via `VICTORY_PIPELINE_BOOK=1`
- `victory run <exe> --console series-s --execute` — launch the title under the compatibility layer

---

## Observed Verification & Benchmarks

### Initial Baseline Benchmarks & Policy (`--profile diag` Historical Captures)

Run the current verification suite locally after building (`--locked`):
- `cargo run --locked -p xtask -- policy`
- `cargo test --locked -p xtask`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --locked`
- `cargo build --locked --release -p victory`

`victory-gpu` unit coverage includes `low_vram_uses_live_budget_without_hurting_igpu_or_8gb`, `non_local_heaps_do_not_starve_texture_admission_on_4gb`, `views_shift_down_by_the_levels_left_out`, `a_kept_side_bounds_the_levels_dropped`, `shortened_view_mip_range_fits_the_host_texture`, `slots_a_draw_names_are_handed_out_whatever_their_turn`, and `a_bake_draw_names_the_indices_of_its_constants_and_records`.

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
2. **Render-Target Aliasing (`resource.rs`, `host.rs`)**: Series S open-world render targets consume ~`1.35–2 GiB` (`VICTORY_CARD_ROOM_FOR_TARGETS_MB=1350` default on `<= 6 GiB` discrete GPUs). Aliasing transient G-buffer/shadow render targets with non-overlapping lifetimes is the biggest remaining lever to eliminate WDDM PCIe shared-memory spills.
3. **Complete Wave64 -> Wave32 Compute Lowering (`victory-shader/src/emit/`)**: Remaining dropped compute hashes (`369cbe906ea72764`, `02384a60fda6f2d0`, `6b82bc7d22016cb5`, `5dc3b25986622949`, `6f225b641c5b0708`) fail Wave32 lowering under the safe path; finishing symbolic Wave64 reduction/lane-mask lowering avoids dropped compute dispatches cleanly without `VICTORY_WAVE_WIDE` TDR risk.
4. **Test Pool vs. Committed Mips & Pre-Warm Caches**: Benchmark `VICTORY_TEXTURE_POOL=0` vs `1` (4 MiB blocks) alongside `8 MiB` `tiles` heaps on open-world saves, and pre-warm with `victory shaders <game-dir>` + `VICTORY_PIPELINE_BOOK=1` + `victory warm`.

See also [`docs/findings.md`](docs/findings.md), [`docs/architecture.md`](docs/architecture.md), [`docs/fork-ray-tracing.md`](docs/fork-ray-tracing.md), [`docs/bug-journal.md`](docs/bug-journal.md), and [`docs/replay.md`](docs/replay.md).

---

## Credits & Upstream Acknowledgments

`potato-recomp` builds directly on the open-source research and compatibility-layer engineering of the following repositories and contributors (GPL-3.0-or-later; see [`LICENSE`](LICENSE)):

- **Upstream Foundation:** [`Marvisak/VIctory`](https://github.com/Marvisak/VIctory) — core loader, PE/format decoders, HLE modules, D3D12 runtime, and shader recompiler
- **4 GB RTX 3050 Profiling & Visual Fixes:** [`Dathinel/victory-4gb`](https://github.com/Dathinel/victory-4gb) (`coolfox-4gb`) — RTX 3050 4 GB benchmarks, `VICTORY_AMBIENT_MARCH` (`PR #15`), `VICTORY_AMBIENT_MOST`, `VICTORY_TEXTURE_KEEP_SIDE`, `scripts/rt_work_off_hashes.txt`, Series-S `VICTORY_UPSCALER=6` + `VICTORY_RENDER_SCALE=0.5` measurements, and `VICTORY_CARD_STEPS` isolation
- **High-Throughput Sync & No-RT Base:** [`Belobug/VIctory-Belobug-AMD`](https://github.com/Belobug/VIctory-Belobug-AMD) (`beloooo`, `romainhedouin`) — Renderer Ports 1/5/6/7, ambient settling, parallel prelude uploads, buffer sweep, `RetailPlane` compose, and pipeline workers
- **Windows Pareto-Residency Base:** [`codec-d/cool_fox`](https://github.com/codec-d/cool_fox) & continuation — Windows residency framework, Wave32 lowering, `port_descriptors`, two-plane compose, and DXIL fixes
- **Descriptor Table Hooks, Render Scale & Telemetry:** [`avatarmanz/VIctory-7900XTX`](https://github.com/avatarmanz/VIctory-7900XTX) & [`ZigZagBoi1/ScarlettPort`](https://github.com/ZigZagBoi1/ScarlettPort) — `COPY_CALLS` / `COPY_128` descriptor copy hooks, render-scale patches, `VICTORY_THIN`, `VICTORY_ASYNC_SUBMIT`, `VICTORY_JOIN_PARTS`, `VICTORY_RELAY`, and `docs/findings.md`
- **Community Forks & Feature Contributors:**
  - `miaohaixiang1-netizen` — `VICTORY_TEXTURE_DROP_MIPS` mip-dropping and `VICTORY_NAN_SCRUB` pixel/image NaN+Inf scrubbing
  - `UwUCammie` (`fxceless/victory-fork#1264`) — `VICTORY_BAKE_SOURCES` synchronous skin-bake texture preloading
  - [`trustno1-coder/VIctory-NVIDIA`](https://github.com/trustno1-coder/VIctory-NVIDIA) — `scripts/rt_off_hashes.txt` ray-tracing pass skip list and air froxel NaN clamping
  - `星记` (`starnotes`) — `VICTORY_ZERO_RELOOK_FRAMES` early zero-read retry
  - `kelvaroq` — `plaincopy` (`VICTORY_COPY_PLAIN=1`) fast guarded memory copies
  - `uiliannn` ([`uiliannn/VItoriaBr`](https://github.com/uiliannn/VItoriaBr)) — capped-heap `VICTORY_DYNAMIC_SLOTS` override and unused table UAV writeback skipping
  - [`fxceless/victory-fork`](https://github.com/fxceless/victory-fork) (`fxceless`) — texture readback, small card buffers, buffer-byte hashing, fast-math bitflags (`VICTORY_SHADER_FAST_BITS`), [`docs/README-fxceless.md`](docs/README-fxceless.md)
  - [`DexNom/VIctory`](https://github.com/DexNom/VIctory) — ray-dispatch thread counts, cube gradients, sparse range coalescing
  - [`FQY55kai/ZJOU-VICTORY`](https://github.com/FQY55kai/ZJOU-VICTORY) & [`Linox-IDK/LNX-victory`](https://github.com/Linox-IDK/LNX-victory) — render-target/pipeline bind cache (`VICTORY_BIND_CACHE`) & shader fixes
  - [`myokex/victory-amd`](https://github.com/myokex/victory-amd) (`Spexivus`, `joroyt`, `tightfall`, `Joshua Odom`)
  - Individual community contributors credited in source history: `Marvis`, `Duff`, `codec_d`, `fxceless`, `Geckomacabre`, `cxy0072019-sketch`, and all contributors recorded in upstream commit logs.
- **Reference Layers & Tooling Studied Upstream:**
  - [`ivall/xcl`](https://github.com/ivall/xcl) (`GPL-2.0-only` console instruction/runtime reference)
  - [`ZSlayerHQ/victory-world-runtime`](https://github.com/ZSlayerHQ/victory-world-runtime), [`Zettrox/ztrx`](https://github.com/Zettrox/ztrx), [`R0Y00/Xbox2Windows`](https://github.com/R0Y00/Xbox2Windows)
  - [Microsoft DirectX Shader Compiler (`DXC`)](https://github.com/microsoft/DirectXShaderCompiler)
