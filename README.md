# Termina (Rust)

Termina is an experimental Nintendo Switch 2 and Switch 1 emulator, written in Rust. Switch 2 titles are the priority; Switch 1 titles share the same pipeline. Rendering is Vulkan only.

Only use games, keys and firmware you have legally obtained. None are included.

## Subsystems

| Crate | Path | Description |
|---|---|---|
| `lichii-common` | `crates/common` | Address types, Horizon result codes, settings model, logging, paths |
| `lichii-memory` | `crates/memory` | Guest physical memory, guest address spaces, exclusive monitor |
| `lichii-cpu` | `crates/cpu` | A64 decoder, interpreter, JIT, `CpuCore` |
| `lichii-crypto` | `crates/crypto` | Key sets (`prod.keys`, `title.keys`), AES-CTR/XTS, key derivation |
| `lichii-fs` | `crates/fs` | VFS, NCA, PFS0/NSP, HFS0/XCI, RomFS, NACP, CNMT, save data |
| `lichii-loader` | `crates/loader` | NSO/NRO/NPDM, loose ExeFS, title metadata, program images |
| `lichii-kernel` | `crates/kernel` | HLE Horizon kernel: processes, threads, scheduler, SVCs, IPC |
| `lichii-services` | `crates/services` | HLE system services (sm, fs, am, hid, vi, nvdrv, gsv, audio, ...) |
| `lichii-gpu` | `crates/gpu` | GPFIFO, DMA pusher, engines (Maxwell/Ampere), macros, GMMU, host1x |
| `lichii-texture` | `crates/texture` | Surface formats, block-linear tiling, ASTC/BCn decoders |
| `lichii-video` | `crates/video` | FFmpeg video decoding, NVDEC codec state, output image conversion |
| `lichii-shader` | `crates/shader` | Maxwell/Ampere SASS -> IR -> SPIR-V |
| `lichii-vulkan` | `crates/vulkan` | Vulkan renderer, caches, pipelines, resolution scaling, presentation |
| `lichii-audio` | `crates/audio` | Audio renderer HLE and host output |
| `lichii-input` | `crates/input` | Host controllers and keyboard, mapping to npad state |
| `lichii-core` | `crates/core` | `System`: wires every subsystem together and boots a title |
| `lichii` | `crates/frontend` | Desktop application frontend: windowing, game list, settings UI |

## Building

```sh
cargo build --release
cargo test --workspace
```

### Requirements

- **Rust:** Rust 1.98 or newer (stable toolchain).
- **CMake & C Compiler:** CMake and Visual Studio C++ Build Tools (Windows) or GCC/Clang (Linux).
- **Vulkan:** Vulkan SDK and drivers supporting Vulkan 1.3+.
- **Audio:** On Linux, ALSA development headers (`libasound2-dev`).

### Video Decoding (FFmpeg)

Termina emulates hardware video decoding (NVDEC) using FFmpeg (`libavcodec`). FFmpeg is loaded dynamically at runtime:
- **Windows:** Place `avcodec-61.dll` and `avutil-59.dll` in the executable folder or working directory.
- **Linux:** Install distribution packages (`libavcodec60` on Ubuntu 24.04, `ffmpeg-libs` on Fedora).
- Alternatively, set the environment variable `LICHII_FFMPEG_DIR` to point to the directory containing the shared libraries.

## License

GPL-3.0-or-later, see [LICENSE](LICENSE). This project inherits and builds upon the work of the Lichii, Eglantine, Eden, yuzu, and Sudachi projects and their contributors.
