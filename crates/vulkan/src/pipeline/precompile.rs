//! Pre-launch shader translation and Vulkan pipeline pre-warming.
//!
//! Translates standalone shaders extracted from a title's RomFS into `shaders.bin` and builds
//! all cached graphics and compute pipelines into `pipelines.bin` before gameplay starts.

use std::cell::Cell;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use lichii_common::GpuVAddr;
use lichii_common::settings::{GraphicsSettings, ResolutionScale};
use lichii_shader::backend::Bindings;
use lichii_shader::cache::{TRANSLATOR_ID, compile as compile_cached};
use lichii_shader::info::TexturePixelFormat;
use lichii_shader::ir::TextureType;
use lichii_shader::translate::TranslateOptions;
pub use lichii_shader::{RuntimeInfo, ShaderArch, ShaderEnvironment, ShaderStage};

use super::cache::PipelineCache;
use super::shader_disk::ShaderDisk;
use crate::device::Device;
use crate::error::Result;

const SHADER_CACHE_FILE: &str = "shaders.bin";
const STAMP_FILE: &str = "precompiled.stamp";

/// Returns `true` when `cache_dir` was already prepared by the current shader translator.
pub fn is_precompiled(cache_dir: &Path) -> bool {
    std::fs::read(cache_dir.join(STAMP_FILE))
        .is_ok_and(|bytes| bytes == TRANSLATOR_ID.to_le_bytes())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrecompilePhase {
    Translating,
    BuildingPipelines,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrecompileProgress {
    pub phase: PrecompilePhase,
    pub done: usize,
    pub total: usize,
    pub translated: usize,
    pub pipelines_built: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PrecompileSummary {
    pub translated: usize,
    pub pipelines_queued: usize,
    pub cancelled: bool,
}

struct StaticRomfsEnv<'a> {
    arch: ShaderArch,
    stage: ShaderStage,
    code: &'a [u8],
    hit_unknown: Cell<bool>,
}

impl ShaderEnvironment for StaticRomfsEnv<'_> {
    fn arch(&self) -> ShaderArch {
        self.arch
    }

    fn stage(&self) -> ShaderStage {
        self.stage
    }

    fn start_address(&self) -> GpuVAddr {
        0x1000_0000
    }

    fn read_u64(&self, address: GpuVAddr) -> u64 {
        let Some(offset) = address
            .checked_sub(self.start_address())
            .and_then(|o| usize::try_from(o).ok())
        else {
            self.hit_unknown.set(true);
            return 0;
        };
        if let Some(bytes) = self.code.get(offset..offset + 8) {
            u64::from_le_bytes(bytes.try_into().expect("8 bytes"))
        } else {
            0
        }
    }

    fn read_cbuf(&self, _index: u32, _offset: u32) -> u32 {
        self.hit_unknown.set(true);
        0
    }

    fn shared_memory_size(&self) -> u32 {
        0
    }

    fn workgroup_size(&self) -> [u32; 3] {
        [1, 1, 1]
    }

    fn texture_type(&self, _raw_handle: u32) -> TextureType {
        self.hit_unknown.set(true);
        TextureType::Color2D
    }

    fn texture_pixel_format(&self, _raw_handle: u32) -> TexturePixelFormat {
        self.hit_unknown.set(true);
        TexturePixelFormat::A8b8g8r8Unorm
    }

    fn is_texture_pixel_format_integer(&self, _raw_handle: u32) -> bool {
        self.hit_unknown.set(true);
        false
    }

    fn viewport_transform_state(&self) -> u32 {
        1
    }

    fn texture_bound_buffer(&self) -> u32 {
        0
    }

    fn local_memory_size(&self) -> u32 {
        0
    }

    fn has_hle_macro_state(&self) -> bool {
        false
    }
}

/// Translates standalone `extracted` shaders into `cache_dir/shaders.bin` and prebuilds all
/// cached pipelines into `cache_dir/pipelines.bin`.
pub fn precompile_title(
    device: &Arc<Device>,
    settings: &GraphicsSettings,
    cache_dir: &Path,
    arch: ShaderArch,
    extracted: &[(ShaderStage, Vec<u8>)],
    cancel: &AtomicBool,
    on_progress: impl Fn(PrecompileProgress),
) -> Result<PrecompileSummary> {
    std::fs::create_dir_all(cache_dir)?;
    let mut disk = ShaderDisk::open(cache_dir.join(SHADER_CACHE_FILE))?;

    let mut translated = 0usize;
    let total_shaders = extracted.len();
    for (idx, (stage, code)) in extracted.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            disk.flush()?;
            return Ok(PrecompileSummary {
                translated,
                pipelines_queued: 0,
                cancelled: true,
            });
        }
        if idx % 16 == 0 || idx + 1 == total_shaders {
            on_progress(PrecompileProgress {
                phase: PrecompilePhase::Translating,
                done: idx,
                total: total_shaders,
                translated,
                pipelines_built: 0,
            });
        }
        let env = StaticRomfsEnv {
            arch,
            stage: *stage,
            code,
            hit_unknown: Cell::new(false),
        };
        let options = TranslateOptions {
            profile: &device.shader_profile,
            host: &device.shader_host,
            resolution_scale: ResolutionScale::X1,
        };
        let mut bindings = Bindings::default();
        if let Ok(entry) = compile_cached(&env, options, &RuntimeInfo::default(), &mut bindings)
            && !env.hit_unknown.get()
        {
            disk.append(&entry);
            translated += 1;
        }
    }
    disk.flush()?;
    let modules = disk.modules();

    let mut warm_settings = settings.clone();
    warm_settings.async_shader_compilation = true;
    warm_settings.disk_shader_cache = true;
    let mut pipelines = PipelineCache::new(device, &warm_settings, Some(cache_dir), None)?;
    let queued = pipelines.prebuild_all(|hash| modules.get(&hash).cloned());

    loop {
        let remaining = pipelines.building() as usize;
        let built = queued.saturating_sub(remaining);
        on_progress(PrecompileProgress {
            phase: PrecompilePhase::BuildingPipelines,
            done: built,
            total: queued,
            translated,
            pipelines_built: built,
        });
        if remaining == 0 || cancel.load(Ordering::Relaxed) {
            break;
        }
        std::thread::sleep(Duration::from_millis(40));
    }

    let cancelled = cancel.load(Ordering::Relaxed);
    if !cancelled {
        pipelines.wait_for_builds();
    }
    pipelines.save()?;
    if !cancelled {
        let _ = std::fs::write(cache_dir.join(STAMP_FILE), TRANSLATOR_ID.to_le_bytes());
    }

    Ok(PrecompileSummary {
        translated,
        pipelines_queued: queued,
        cancelled,
    })
}
