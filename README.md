# potato-recomp

`potato-recomp` is a low-VRAM, No-Ray-Tracing compatibility and performance fork of the Xbox Series -> Windows Direct3D 12 translation runtime (`VIctory`). It adapts upstream high-VRAM runtime work to run modern AAA game builds on **4 GB discrete GPUs** and **shared-memory iGPUs / APUs** using the `--console series-s` profile, while preserving indoor lighting and preventing out-of-memory device removals.

> **Legal / Repository Policy Notice**
>
> This repository contains **only** clean-room compatibility runtime source code, build configuration, documentation, and the existing shader helper table (`crates/victory-formats/src/bcpack-tables.bin`). No copyrighted AAA game binaries, dumps, keys, shaders, or assets are included or accepted. All paths and scripts expect you to supply your own legitimate console build externally (`VICTORY_GAME_DIR` / `VICTORY_GAME_EXE`).
> Run `cargo xtask policy` before committing to verify repository hygiene.

---

## Key Fixes Over Upstream (`4 GB` / No-RT Focus)

1. **No-RT Indoor Lighting & Volumetric Air Capping (`crates/victory-gpu/src/ambient.rs`)**
   - Lowered `DEFAULT_FLOOR` (`VICTORY_AMBIENT_MIN`, set `0`/`off` to disable) to `0.025` so loading transitions and shaded interiors stay legible without dark blackouts when `VICTORY_RT_RESOLVE=0` and `VICTORY_ACTIVATE_RAYS=0`.
   - Capped volumetric air scattering inside `clear_air()` (`float cap = air * avg;`, configured with `VICTORY_AMBIENT=0.8` and `VICTORY_AMBIENT_AIR=1.5`) to eliminate washed-out milky indoor haze.
2. **Accurate Skipped-Pipeline Diagnostics (`crates/victory-gpu/src/pipeline/make.rs`)**
   - Updated `missing()` to report `p.left_out` (for example, ray-tracing hit/trace pipelines skipped when BVH building is off) instead of misreporting skipped pipelines as `"being made"`.
3. **Pool Texture Aliasing Barrier Crash Fix (`crates/victory-gpu/src/pool.rs`, `crates/victory-gpu/src/texture.rs`)**
   - When disabling `pool::TEXTURES` via `VICTORY_TEXTURE_POOL=0` to recover fragmented VRAM (`752 MB` dead holes eliminated on 4 GB cards), committed non-pooled textures previously triggered an invalid D3D12 aliasing barrier and crashed `ID3D12GraphicsCommandList::Close()` with `0x80070057`.
   - Added `POOLED` tracking and `pool::pooled(&ID3D12Resource)` so `record_pool_aliasing_barrier` skips committed textures (`aliasing_barrier_skips_committed_textures`). Also uses `LOW_VRAM_BLOCK` (`4 MiB`) and `LOW_MEM_BLOCK` (`16 MiB`) block sizes instead of `64 MiB` on memory-constrained hosts.
4. **Low-VRAM Room Budgeting (`crates/victory-gpu/src/host.rs`, `crates/victory-gpu/src/resource.rs`)**
   - Added cached `Host::dedicated_video_memory()` (via `IDXGIAdapter1::GetDesc1()`) and `resource::room_used_limit_bytes(budget, dedicated, room_percent)`.
   - On discrete GPUs with `<= 6 GiB` dedicated VRAM, `room()` checks `by_budget.max(dedicated / 100 * pct)` (`3,891 MiB` at `95%` on a `4,096 MiB` card) instead of stalling prematurely at `95%` of WDDM's shrunk `3,370 MB` local budget, while `let_go_idle()` keeps evicting against DXGI `budget`. APUs/iGPUs and `8+ GiB` GPUs remain governed strictly by DXGI `budget`.
5. **Tuned 4 GB No-RT Launcher (`Play-Potato-4GB.cmd`)**
   - Bundles the validated settings: `VICTORY_TEXTURE_POOL=0`, `VICTORY_LEVELS_ON_DEMAND=1`, `VICTORY_LEVELS_MIN_KB=512`, `VICTORY_CARD_ROOM_FOR_TARGETS_MB=1280`, `VICTORY_CARD_ROOM_FOR_BUFFERS=0`, `VICTORY_CARD_BUFFERS_MB=96`, `VICTORY_CARD_BUFFERS_MIN_MB=2`, `VICTORY_TEXTURE_ROOM_PERCENT=95`, `VICTORY_PORT_DESCRIPTORS=1`, `VICTORY_THIN=1`, `VICTORY_ASYNC_SUBMIT=1`, and `VICTORY_INDIRECT_REST=0`.

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

Set `VICTORY_GAME_DIR` (and optionally `VICTORY_GAME_EXE`; otherwise the launcher selects the first `.exe` in `VICTORY_GAME_DIR`), then run [`Play-Potato-4GB.cmd`](Play-Potato-4GB.cmd):

```cmd
set "VICTORY_GAME_DIR=C:\Path\To\AAA.Game.Dump"
Play-Potato-4GB.cmd
```

By default, cache, saves, and logs go under `%LOCALAPPDATA%\VIctory\`. See [`victory.example.toml`](victory.example.toml) for persistent TOML configuration or [`Play-VIctory.cmd`](Play-VIctory.cmd) for the upstream Pareto-residency profile.

Useful CLI subcommands (`target\release\victory.exe --help`):

- `victory inspect <exe> [--imports] [--exports]` — inspect PE headers and console imports
- `victory census <paths...>` — check HLE coverage of imported DLLs/symbols
- `victory shaders <game-dir>` — pre-translate shader containers into the local cache
- `victory translate <paths...> [--validate] [--pipelines]` — test shader lowering & DXIL validation
- `victory warm` — pre-compile D3D12 PSOs recorded via `VICTORY_PIPELINE_BOOK=1`
- `victory run <exe> --console series-s --execute` — launch the title under the compatibility layer

---

## Observed Tests

All commands below were executed on Windows after the final source edits and sanitization using `--locked` (`asset policy: 253 files ok`, DMCA/path regex scan `0 matches`).

### Policy & Release Build

| Check | Command | Result |
|---|---|---|
| Asset & file allowlist | `cargo xtask policy` | Passed (`asset policy: 253 files ok`) |
| Release binary | `cargo build --locked --release -p victory` | Passed (`49.44s`) |

### Workspace Test Suite (`cargo test --workspace --locked --no-fail-fast`)

| Crate / Target | Passed | Failed | Ignored | Time | Notes |
|---|---:|---:|---:|---|---|
| `victory` (bin unit tests) | 10 | 0 | 0 | `0.05s` | CPU checks, census, repo-path guards |
| `victory` (`tests/census.rs`) | 2 | 0 | 0 | `0.59s` | CLI census integration tests |
| `victory-core` | 13 | 0 | 0 | `0.00s` | Config & repo-boundary policy tests |
| `victory-formats` | 41 | 0 | 5 | `8.38s` | Texture swizzling, BCPack, XMA (5 external-ref/bench ignored) |
| `victory-game` | 3 | 0 | 0 | `0.00s` | `sgd5` container tests |
| `victory-gpu` (default `--lib`) | 338 | **2** | 34 | `15.68s` | Includes `low_vram_room_tests` & `aliasing_barrier_skips_committed_textures` passing; see note below |
| `victory-gpu` (`replay`, `replay_big` default) | 0 | 0 | 0 | `0.00s` | Gated behind `diagnostics` feature |
| `victory-hle` (`--lib`) | 150 (+1 isolated) | 0 | 2 | `1.23s` | `loading_a_host_dll_does_not_block_thread_attach` runs isolated (`+0.51s`); 2 benchmarks ignored |
| `victory-hle` (`tests/no_dll_planting.rs`) | 1 | 0 | 0 | `0.00s` | Safe DLL loading |
| `victory-loader` (`--lib`) | 42 | 0 | 0 | `0.58s` | `descriptor_sites`, `write_sites`, TLS, instruction emulation |
| `victory-loader` (`tests/load_probe.rs`, `title_dlls.rs`) | 3 | 0 | 0 | `0.69s` | `2 + 1` passed |
| `victory-pe` | 36 | 0 | 0 | `0.00s` | PE32+ and import library parser |
| `victory-pipeline` | 7 | 0 | 0 | `0.08s` | Content-addressed converter cache |
| `victory-shader` | 292 | 0 | 4 | `291.73s` | Debug IR/lowering/Wave64 tests (4 tool/bench/strict ignored) |
| `xtask` | 6 | 0 | 0 | `0.36s` | Repository policy enforcement unit tests |

### Diagnostics Feature Suite (`cargo test --locked -p victory-gpu --features diagnostics --no-fail-fast`)

| Target | Passed | Failed | Ignored | Time | Notes |
|---|---:|---:|---:|---|---|
| `victory-gpu` (`diagnostics --lib`) | 388 | **2** | 34 | `16.30s` | Covers `ambient::tests` (`roles_by_name`, `settings`, `the_passes_build`) & `pipeline::make::tests` (`skipped_pipelines_report_their_left_out_reason`) |
| `victory-gpu` (`tests/replay.rs`) | 1 | 0 | 0 | `1.26s` | `a_frame_is_recorded_and_replayed` |
| `victory-gpu` (`tests/replay_big.rs`) | 0 | 0 | 1 | `0.00s` | Ignored (multi-GB capture requirement) |

> **Pre-existing Failure Note (`victory-gpu`):** The 2 failures in both default and `diagnostics` builds are pre-existing upstream WARP indirect-execution tests in `crates/victory-gpu/src/indirect.rs`:
> - `indirect::tests::host_list_budget_starts_empty_at_each_submission` (`indirect.rs:2744`, `left: 1, right: 0`)
> - `indirect::tests::warp_batch_charges_every_execution` (`indirect.rs:3195`, `left: 0, right: 134217728`)

---

## Observed Low-VRAM Benchmarks (`diag` Instrumented Captures)

**Test System:** `AMD Ryzen 7 5800H`, `NVIDIA GeForce RTX 3050 Laptop GPU` (`4,096 MiB` dedicated VRAM, `3,370 MB` DXGI local budget, `7,212 MB` shared memory), `16 GB` system RAM, Windows 11, `--console series-s`, No-RT (`VICTORY_RT_RESOLVE=0`, `VICTORY_ACTIVATE_RAYS=0`, `VICTORY_RT_TRACE=1`).

> **Important:** All benchmark metrics below come from **`--profile diag --features diagnostics`** captures (`250–270 s` sessions writing frame PNGs and heavy telemetry). Instrumented `diag` captures run much slower than `--release` builds due to per-frame readbacks, PNG dumps, and counter overhead; treat `cap_*` comparisons as **apples-to-apples tuning telemetry**, not `--release` ceiling FPS.

| Run | Key Configuration | FPS / Frame Time (`diag`) | Host Texture Copies Created | Blocked Passes | `tiles` Heaps | Peak/Steady Card & Shared | Stability & Visual Outcome |
|---|---|---|---|---:|---:|---|---|
| `cap_fixed` | `POOL=4M`, `ROOM=85%`, `CARD_BUFFERS=288`, `MIN_KB=512`, `TARGETS=0` | `1.8 fps`, `569.0 ms/f` (`226.0` prep, `302.4` GPU, `112.3` back), `4,584` draws/f | `23,678` without copy | `1,119` | — | `4,177 MB` card, `5,391 MB` shared | `1,748 MB` pinned in `pool::TEXTURES` for `996 MB` live (`752 MB` dead holes); black characters/rooms |
| `cap_pool0` | `POOL=0`, `ROOM=95%`, `CARD_BUFFERS=288`, `MIN_KB=512`, `TARGETS=0` | `1.2 -> 0.9 fps`, `1074.9 ms/f` (`205.9` prep, `727.3` GPU, `148.9` back), `4,624` draws/f | `21,027` created (`10,550` alive, `1,249 MB`) | `408` (`11,889` missing) | — | `3,743 MB` card, `5,341 MB` shared | Crisp indoor lighting & exteriors; main character sharp, secondary character head still black |
| `cap_tuned` | `POOL=0`, `MIN_KB=128`, `CARD_BUFFERS=64`, `BUDGET=4000`, `ROOM=95%` | `0.3 fps`, `3582.7 ms/f`, `7,128` draws/f | `32,311` created (`22,106` due slots) | `0` | `832 MB` (`12,313` made) | `3,807 MB` card, `4,681 MB` shared | **Regression:** `MIN_KB=128` caused `heap buffers` spikes of `442–3,703 ms` |
| `cap_clean` | `POOL=0`, `MIN_KB=512`, `CARD_BUFFERS=288`, `BUDGET=4050`, `ROOM=95%` | `119` PNGs up to `frame-1730` | `27,049` created | `0` | — | `4,684 MB` (peak `4,978 MB`), `5,760 MB` shared | All visible faces crisp, **crashed at `227s` (`frame-1735`)** with `0x887A0005` (`DXGI_ERROR_DEVICE_REMOVED`) |
| `cap_sweet` | `POOL=0`, `MIN_KB=512`, `CARD_BUFFERS=128`, `BUDGET=1750`, `ROOM=95%` | `1.6 fps`, `613.7 ms/f` (`262.1` prep, `282.9` GPU, `172.2` back), `4,583` draws/f | `25,991` created | `353` | `320 MB` | `3,748 MB` card, `4,962 MB` shared | Stable run with modest missing-texture pressure |
| `cap_gold` | `POOL=0`, `MIN_KB=512`, `CARD_BUFFERS=96`, `BUDGET=1950`, `ROOM=85%` | `1.0 fps`, `1042.9 ms/f` (`130.9` GPU) | `24,294` created (`5` copies `80 MB` card buffers) | `256` | — | `3,692 MB` card, `4,963 MB` shared | Stable run, low GPU ms, manual budget still slightly conservative |
| `cap_verified` | Auto `room_used_limit_bytes`, `TARGETS=1024`, `CARD_BUFFERS=96`, `ROOM=95%`, `POOL=0`, `MIN_KB=512` | `1.3 fps`, `773.7 ms/f` (`486.1` prep, `248.9` GPU, `304.8` back), `3,640` draws/f | `32,692` created (`4` copies `80 MB` card buffers) | `394` | `448 MB` | `3,769 MB` card, `4,740 MB` shared | `198` PNGs (`frame-2915`, `270.5s`, **0 crashes**) |
| `cap_verified2` | Same as above + `TARGETS_MB=1280` | `1.1 fps`, `897.8 ms/f` (`316.4` prep, `211.6` GPU, `241.7` back), `3,648` draws/f | **`34,056` created** | `283` | `320 MB` (`1` heap freed) | **`3,647 MB` card, `4,570 MB` shared** | `175` PNGs (`frame-2576`, `250.2s`, **0 crashes**), lowest shared footprint, up to `2,200` more host copies recovered than `cap_clean` before crash |

---

## Future Developer Guide: Optimizing for Low-End Hardware (`4 GB` GPUs & iGPUs / APUs)

If you want to push open-world FPS closer to **30+ FPS** on budget laptops and integrated GPUs, our telemetry and code inspection show the main bottlenecks are no longer static VRAM headroom alone — **CPU submission preparation, `64 MiB` heap granularity, and non-evictable render targets** dominate.

### 1. Understand Discrete 4 GB vs. iGPU / APU Behavior
- On discrete 4 GB laptops (`3,370 MB` DXGI local budget), ~`1.3 GiB` is immediately consumed by non-evictable RenderTargets during world spawn (`VICTORY_CARD_ROOM_FOR_TARGETS_MB=1280`), leaving roughly `2.0–2.4 GiB` for shaders, buffers, and mip-streamed host copies (`tiles` + `KINDS[1]`).
- On iGPUs / APUs (AMD Radeon iGPUs sharing system RAM), `Host::dedicated_video_memory()` intentionally falls back to DXGI `budget` via `room_used_limit_bytes` when dedicated VRAM is absent or small (verified down to `512 MiB` dummy dedicated carveouts), preventing artificial throttling while respecting WDDM shared commit.
- However, APUs and low-end CPUs suffer doubly from `preparing` work on the CPU thread because system-RAM bandwidth is shared with the GPU.

### 2. Attack the CPU `preparing` Wall (`205–486 ms` in `diag` / fluctuating GPU usage)
Even when `GPU` work drops to `130.9–248.9 ms` (`cap_gold` / `cap_verified`), `preparing` stays between `205–486 ms/frame` while `copying back` adds `112–305 ms/frame`. That serial CPU stall explains why **CPU cores saturate while GPU utilization fluctuates** on low-end PCs and APUs:
- Keep **`VICTORY_PORT_DESCRIPTORS=1`** (`crates/victory-gpu/src/port_descriptors.rs` + `crates/victory-loader/src/descriptor_sites.rs`) enabled so descriptor table writes are tracked via byte marks rather than scanning ~290 `64 KiB` write-watch pages per frame.
- Keep **`VICTORY_LEVELS_MIN_KB=512`** (`Play-Potato-4GB.cmd`): lowering `VICTORY_LEVELS_MIN_KB` to `128` (`cap_tuned`) inflated due slots to `22,106` and triggered `442–3,703 ms` `heap buffers` stalls (`0.3 fps`).
- **Next engineering targets in `host.rs` / `watch.rs`:**
  - Pipeline/overlap submission preparation across multiple threads (`host.rs`) so the next frame's write-watch / dirty-range scan runs while the GPU executes the current submission (`VICTORY_ASYNC_SUBMIT=1` helps, but `preparing` still gates submission cadence).
  - Detect unified-memory adapters (iGPUs with UMA) and avoid unnecessary sysmem-to-card/card-to-sysmem shadow bounces (`VICTORY_COPIES_IN_PLACE`, `shadow.rs`, `host.rs`).

### 3. Shrink `tiles` Heap Granularity & Pack Mips by Lifetime (`crates/victory-gpu/src/tiles.rs`)
- In `cap_verified2`, `tiles` allocated `320 MB` of `64 MiB` placed heaps, yet could only return **`1` heap** to the card under pressure because a single pinned low-mip tile keeps the whole `64 MiB` heap alive.
- **Next optimization:** Mirror `pool.rs`'s `LOW_VRAM_BLOCK` (`4 MiB`) / `LOW_MEM_BLOCK` (`16 MiB`) strategy inside `tiles.rs` (or segregate persistent mip tails from transient high mips) so `let_go_idle()` frees full physical heaps immediately during world-spawn surges. This is the cleanest way to eliminate the remaining `~283` blocked passes on 4 GB cards without tripping `0x887A0005`.

### 4. Render Target Aliasing & Resolution Scaling
- RenderTargets currently reserve ~`1.3 GiB` statically on Series S (`VICTORY_CARD_ROOM_FOR_TARGETS_MB=1280`). Implementing D3D12 placed-heap aliasing across non-overlapping render-target lifetimes (or exposing an internal render-scale multiplier for depth/G-buffer passes) would reclaim `300–600 MiB` on 4 GB cards and cut pixel/fillrate load on iGPUs.

### 5. Finish Wave64 -> Wave32 Compute Shader Lowering (`crates/victory-shader/src/emit/`)
- Three compute shaders (`02384a60fda6f2d0`, `5dc3b25986622949`, `6f225b641c5b0708`) still fail Wave32 lowering under the conservative path, while forcing `VICTORY_WAVE_WIDE` can hit loop-widening TDRs (`docs/bug-journal.md`). Completing symbolic Wave64 reduction/lane-mask lowering for those 3 hashes eliminates remaining skipped compute work without widening risk.

### 6. Pre-Warm Caches Before Benchmarking
- Always pre-populate shader and driver PSO caches outside timed runs using `victory shaders <game-dir>` and `VICTORY_PIPELINE_BOOK=1` + `victory warm`; cold PSO compilation stalls dominate the first minutes of any capture.

See also [`docs/findings.md`](docs/findings.md), [`docs/architecture.md`](docs/architecture.md), [`docs/fork-ray-tracing.md`](docs/fork-ray-tracing.md), [`docs/bug-journal.md`](docs/bug-journal.md), and [`docs/replay.md`](docs/replay.md).

---

## Credits & Upstream Acknowledgments

`potato-recomp` builds directly on the open-source research and compatibility-layer engineering of the following repositories and contributors (all licensed under their respective upstream open-source terms; see [`LICENSE`](LICENSE), GPL-3.0-or-later):

- **Upstream Foundation:** [`Marvisak/VIctory`](https://github.com/Marvisak/VIctory) — core loader, PE/format decoders, HLE modules, D3D12 runtime, and shader recompiler
- **Direct Fork Base:** [`codec-d/cool_fox`](https://github.com/codec-d/cool_fox) — Windows pareto-residency runtime, Wave32 lowering, `port_descriptors`, two-plane compose, and DXIL fixes
- **Performance & Frame Telemetry:** [`avatarmanz/VIctory-7900XTX`](https://github.com/avatarmanz/VIctory-7900XTX) — `VICTORY_THIN`, `VICTORY_ASYNC_SUBMIT`, `VICTORY_JOIN_PARTS`, `VICTORY_RELAY`, `VICTORY_CARD_STEPS`, and `docs/findings.md`
- **Community Forks & Feature Contributors:**
  - [`fxceless/victory-fork`](https://github.com/fxceless/victory-fork) (`fxceless`) — texture readback, small card buffers, buffer-byte hashing ([`docs/README-fxceless.md`](docs/README-fxceless.md))
  - [`Belobug/VIctory-Belobug-AMD`](https://github.com/Belobug/VIctory-Belobug-AMD) (`beloooo`, `romainhedouin`) — ambient No-RT stand-in foundation, pipeline workers, constant-buffer bookkeeping
  - [`DexNom/VIctory`](https://github.com/DexNom/VIctory) — ray-dispatch thread counts, cube gradients, sparse range coalescing
  - [`FQY55kai/ZJOU-VICTORY`](https://github.com/FQY55kai/ZJOU-VICTORY) — render-target binding & shader fixes
  - [`myokex/victory-amd`](https://github.com/myokex/victory-amd) (`Spexivus`, `joroyt`, `tightfall`, `Joshua Odom`)
  - Individual community contributors credited in source history: `Marvis`, `Duff`, `codec_d`, `fxceless`, `Geckomacabre`, `cxy0072019-sketch`, and all contributors recorded in upstream commit logs.
- **Reference Layers & Tooling Studied Upstream:**
  - [`ivall/xcl`](https://github.com/ivall/xcl) (`GPL-2.0-only` console instruction/runtime reference)
  - [`ZSlayerHQ/victory-world-runtime`](https://github.com/ZSlayerHQ/victory-world-runtime), [`uiliannn/VItoriaBr`](https://github.com/uiliannn/VItoriaBr), [`Zettrox/ztrx`](https://github.com/Zettrox/ztrx), [`R0Y00/Xbox2Windows`](https://github.com/R0Y00/Xbox2Windows)
  - [Microsoft DirectX Shader Compiler (`DXC`)](https://github.com/microsoft/DirectXShaderCompiler)
