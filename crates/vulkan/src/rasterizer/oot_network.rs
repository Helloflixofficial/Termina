//! Bypass for *The Legend of Zelda: Ocarina of Time* (`0400486024DEC000`) post-tonemap
//! anti-aliasing network and sharpening output pass, ported from C++ `VideoCommon::OotNetwork`.
//!
//! When `oot_anti_aliasing == OotAntiAliasing::NoSharpening`:
//! - On update 1.1.1, replaces `OUTPUT_PROGRAM_KEY` (`0xda6df3e54c716df2`) with a same-size
//!   point copy from the latest tonemapped `B10G11R11Float` render target and skips the
//!   heavy prepass, unused dispatch, and network compute passes while the replacement succeeds.
//! - On base 1.0.0, recognizes the six `BASE_OUTPUT_PROGRAMS` before shader translation and
//!   replaces them with a bilinear copy from the tonemapped render target.

use std::collections::HashMap;

use ash::vk;
use lichii_common::GpuVAddr;
use lichii_gpu::backend::ComputeContext;
use lichii_gpu::engines::maxwell_3d::regs::RenderTargetConfig;
use lichii_gpu::gmmu::Gmmu;
use lichii_texture::PixelFormat;
use lichii_texture::block_linear::GobLayout;
use lichii_texture::guest::RenderTargetFormat;
use lichii_texture::tic::TextureMemory;

use crate::error::Result;
use crate::pipeline::FxBuildHasher;
use crate::pipeline::environment::TexturePool;
use crate::texture_cache::{BlitSide, ImageInfo, TextureCache, Transfer};

pub const PROGRAM_ID: u64 = 0x0400_4860_24DE_C000;

pub const PREPASS_PROGRAM_KEY: u64 = 0x8475_d671_e696_7428;
pub const UNUSED_DISPATCH_PROGRAM_KEY: u64 = 0x9387_db79_8781_f081;
pub const NETWORK_PROGRAM_KEY: u64 = 0x4259_3194_bb0b_2b32;
pub const OUTPUT_PROGRAM_KEY: u64 = 0xda6d_f3e5_4c71_6df2;

pub const INPUT_IMAGE_HANDLE_OFFSET: u32 = 0xc0;
pub const OUTPUT_IMAGE_HANDLE_OFFSET: u32 = 0xd0;

#[derive(Clone, Copy)]
struct BaseOutputProgram {
    first_word: u32,
    size: u32,
    key: u64,
}

const BASE_OUTPUT_PROGRAMS: &[BaseOutputProgram] = &[
    BaseOutputProgram {
        first_word: 0x006b_7919,
        size: 0x57a0,
        key: 0x1e49_acb4_91d9_fb59,
    },
    BaseOutputProgram {
        first_word: 0x0039_7919,
        size: 0x6430,
        key: 0x2142_429c_4374_0683,
    },
    BaseOutputProgram {
        first_word: 0x003c_7919,
        size: 0x6690,
        key: 0xd019_39a7_7592_1a6c,
    },
    BaseOutputProgram {
        first_word: 0x0001_7919,
        size: 0x6430,
        key: 0x33c2_7820_0d77_1cd9,
    },
    BaseOutputProgram {
        first_word: 0x003c_7919,
        size: 0x66f0,
        key: 0x83de_ec17_43dd_ee73,
    },
    BaseOutputProgram {
        first_word: 0x0001_7919,
        size: 0x6250,
        key: 0x58bd_d573_3e36_4169,
    },
];

/// FNV-1a over raw Ampere instruction bytes, matching C++ `Shader::Ampere::NativeProgramKey`.
pub(crate) fn native_program_key(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        hash = (hash ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn fnv1a_step(mut hash: u64, words: &[u32; 4]) -> u64 {
    for &w in words {
        for &b in &w.to_le_bytes() {
            hash = (hash ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

/// Port of C++ `Shader::Ampere::ProgramCaptureState` to compute exact Ampere shader extents.
struct ProgramCaptureState {
    exit_setup: [[u32; 4]; 5],
    last_call_target: u32,
    last_forward_target: u32,
    main_exited: bool,
    computed_branch: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaptureResult {
    Continue,
    Complete,
    InvalidTarget,
}

impl ProgramCaptureState {
    const MAX_INSTRUCTIONS: u32 = 65_536;

    const fn new() -> Self {
        Self {
            exit_setup: [[0; 4]; 5],
            last_call_target: 0,
            last_forward_target: 0,
            main_exited: false,
            computed_branch: false,
        }
    }

    fn observe(&mut self, words: [u32; 4], index: u32) -> CaptureResult {
        let opcode = words[0] & 0xFFF;
        if (index as usize) < self.exit_setup.len() {
            self.exit_setup[index as usize] = words;
            if index as usize + 1 == self.exit_setup.len()
                && self.exit_setup.iter().all(|s| (s[3] & 0xc000_01ff) == 0)
                && self.exit_setup[0][0] == 0x0000_734e
                && self.exit_setup[0][1] == 0
                && self.exit_setup[0][2] == 0
                && self.exit_setup[1][0] == 0x0003_7802
                && self.exit_setup[1][2] == 0xf00
                && self.exit_setup[2][0] == 0x0000_7210
                && self.exit_setup[2][1] == 3
                && self.exit_setup[2][2] == 0x07f1_e0ff
                && self.exit_setup[3][1] == 0xff
                && ((self.exit_setup[3][0] == 0x0101_7210 && self.exit_setup[3][2] == 0x007f_e4ff)
                    || (self.exit_setup[3][0] == 0xff01_7224
                        && self.exit_setup[3][2] == 0x000e_0401))
                && words[0] == 0x0000_7357
                && words[1] == 0
                && words[2] == 0
            {
                let target = self.exit_setup[1][1];
                if (target as usize) < self.exit_setup.len() * 16
                    || !target.is_multiple_of(16)
                    || target >= Self::MAX_INSTRUCTIONS * 16
                {
                    return CaptureResult::InvalidTarget;
                }
                self.last_forward_target = self.last_forward_target.max(target / 16);
            }
        }
        if matches!(opcode, 0x944 | 0x947 | 0x945) {
            let low = words[1] & if opcode == 0x947 { !3 } else { !0 };
            let raw = u64::from(low) | (u64::from(words[2] & 0x3_ffff) << 32);
            let relative = if opcode == 0x945 {
                i64::from(words[1] as i32)
            } else {
                ((raw << 14) as i64) >> 14
            };
            let target = i64::from(index) * 16 + 16 + relative;
            if target < 0 || target % 16 != 0 || target >= i64::from(Self::MAX_INSTRUCTIONS) * 16 {
                return CaptureResult::InvalidTarget;
            }
            if opcode == 0x944 {
                self.last_call_target = self.last_call_target.max((target / 16) as u32);
            } else {
                self.last_forward_target = self.last_forward_target.max((target / 16) as u32);
            }
            let cond = (words[0] >> 12) & 0xF;
            if opcode == 0x947
                && self.computed_branch
                && cond == 7
                && target == i64::from(index) * 16
            {
                return CaptureResult::Complete;
            }
        }
        if opcode == 0x949 || opcode == 0x958 {
            self.computed_branch = true;
        }
        if self.computed_branch {
            return CaptureResult::Continue;
        }
        let cond = (words[0] >> 12) & 0xF;
        if opcode == 0x94d && cond == 7 && index >= self.last_forward_target {
            if index >= self.last_call_target {
                return CaptureResult::Complete;
            }
            self.main_exited = true;
        }
        if opcode == 0x950
            && cond == 7
            && self.main_exited
            && index >= self.last_call_target
            && index >= self.last_forward_target
        {
            return CaptureResult::Complete;
        }
        CaptureResult::Continue
    }
}

fn measure_native_key(gmmu: &Gmmu, program: GpuVAddr) -> u64 {
    let mut state = ProgramCaptureState::new();
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for index in 0..ProgramCaptureState::MAX_INSTRUCTIONS {
        let Ok(words) = gmmu.read::<[u32; 4]>(program + u64::from(index) * 16) else {
            return 0;
        };
        hash = fnv1a_step(hash, &words);
        match state.observe(words, index) {
            CaptureResult::Continue => {}
            CaptureResult::Complete => return hash,
            CaptureResult::InvalidTarget => return 0,
        }
    }
    0
}

fn match_base_output_program(gmmu: &Gmmu, program: GpuVAddr) -> Option<u64> {
    let first_word = gmmu.read::<u32>(program).ok()?;
    let mut scratch = Vec::new();
    for candidate in BASE_OUTPUT_PROGRAMS {
        if candidate.first_word != first_word {
            continue;
        }
        scratch.resize(candidate.size as usize, 0);
        if gmmu.read_block(program, &mut scratch).is_err() {
            continue;
        }
        if native_program_key(&scratch) == candidate.key {
            return Some(candidate.key);
        }
    }
    None
}

pub fn is_tonemap_candidate(
    address: u64,
    width: u32,
    height: u32,
    input_width: u32,
    input_height: u32,
    output_address: u64,
) -> bool {
    address != 0
        && address != output_address
        && input_width != 0
        && input_height != 0
        && width == input_width
        && height == input_height
}

pub fn should_skip_unused_dispatch(
    native_key: u64,
    output_replaced: bool,
    output_pending: bool,
) -> bool {
    native_key == UNUSED_DISPATCH_PROGRAM_KEY && output_replaced && output_pending
}

#[derive(Default)]
pub(crate) struct OotNetworkBypass {
    latest_draw: Option<RenderTargetConfig>,
    latest_address: u64,
    input_width: u32,
    input_height: u32,
    output_address: u64,
    output_replaced: bool,
    output_pending: bool,
    after_network: u64,
    cached_generation: u64,
    native_keys: HashMap<GpuVAddr, u64, FxBuildHasher>,
    logged_stale_skip: bool,
    logged_unused_skip: bool,
    logged_replaced: bool,
    logged_unexpected: bool,
}

impl OotNetworkBypass {
    /// Records `rt0` when it looks like the B10G11R11 tonemap draw before the sharpening pass.
    pub(crate) fn track_render_target(&mut self, target: &RenderTargetConfig) {
        let addr = target.address();
        if target.format != RenderTargetFormat::B10G11R11Float as u32
            || target.tile_mode.is_pitch_linear() != 0
            || self.latest_address == addr
            || !is_tonemap_candidate(
                addr,
                target.width,
                target.height,
                self.input_width,
                self.input_height,
                self.output_address,
            )
        {
            return;
        }
        self.latest_draw = Some(*target);
        self.latest_address = addr;
    }

    fn classify_program(&mut self, gmmu: &Gmmu, program: GpuVAddr, generation: u64) -> (u64, bool) {
        if let Some(base_key) = match_base_output_program(gmmu, program) {
            return (base_key, true);
        }
        if self.cached_generation != generation {
            self.cached_generation = generation;
            self.native_keys.clear();
        }
        let key = *self
            .native_keys
            .entry(program)
            .or_insert_with(|| measure_native_key(gmmu, program));
        let base_output = BASE_OUTPUT_PROGRAMS.iter().any(|c| c.key == key);
        (key, base_output)
    }

    /// Checks whether this native Ampere compute dispatch is part of OoT's post-tonemap network;
    /// returns `Ok(true)` if the dispatch was skipped or replaced by a blit of the tonemapped image.
    pub(crate) fn try_bypass(
        &mut self,
        ctx: &ComputeContext<'_>,
        gob_layout: GobLayout,
        shader_generation: u64,
        textures: &mut TextureCache,
        tx: &mut Transfer<'_>,
    ) -> Result<bool> {
        let (native_key, base_output) =
            self.classify_program(ctx.memory, ctx.program, shader_generation);
        if native_key == 0 {
            return Ok(false);
        }
        if native_key == PREPASS_PROGRAM_KEY {
            if self.output_pending && self.output_replaced {
                self.output_pending = false;
                self.output_replaced = false;
                if !self.logged_stale_skip {
                    self.logged_stale_skip = true;
                    tracing::info!(
                        after_network = format_args!("{:016x}", self.after_network),
                        "Ocarina of Time: the network's output pass was not replaced; running the network"
                    );
                }
            } else {
                self.output_pending = false;
            }
            let skip = self.output_replaced;
            self.output_pending = skip;
            return Ok(skip);
        }
        if should_skip_unused_dispatch(native_key, self.output_replaced, self.output_pending) {
            if !self.logged_unused_skip {
                self.logged_unused_skip = true;
                tracing::info!(
                    shader = format_args!("{native_key:016x}"),
                    "Ocarina of Time: skipping unused AA dispatch after output replacement"
                );
            }
            return Ok(true);
        }
        if native_key == NETWORK_PROGRAM_KEY {
            self.after_network = 0;
            return Ok(self.output_replaced);
        }
        if native_key != OUTPUT_PROGRAM_KEY && !base_output {
            if self.after_network == 0 {
                self.after_network = native_key;
            }
            return Ok(false);
        }

        let Some(cb0) = ctx.const_buffers.first().filter(|cb| cb.enabled) else {
            self.output_replaced = false;
            return Ok(false);
        };
        let pool = TexturePool {
            address: ctx.texture_headers,
            limit: ctx.texture_header_limit,
            via_header_index: ctx.linked_samplers,
            switch2: true,
        };
        let read_tic = |offset: u32| {
            let handle = ctx
                .memory
                .read::<u32>(cb0.address + u64::from(offset))
                .ok()?;
            let entry = pool.entry(ctx.memory, handle)?;
            let desc = entry.decode().ok()?;
            let info = ImageInfo::from_texture(&desc, gob_layout)?;
            Some((desc.address, desc, info))
        };

        let input = read_tic(INPUT_IMAGE_HANDLE_OFFSET);
        let output = read_tic(OUTPUT_IMAGE_HANDLE_OFFSET);

        let tonemapped = self.latest_draw.take();
        self.latest_address = 0;

        let valid_tex = |tex: &(u64, lichii_texture::tic::TextureDescriptor, ImageInfo)| {
            tex.0 != 0
                && matches!(tex.1.memory, TextureMemory::BlockLinear { .. })
                && tex.1.width != 0
                && tex.1.height != 0
        };
        let images_valid = matches!(
            (&input, &output),
            (Some(inp), Some(out))
                if valid_tex(inp)
                    && valid_tex(out)
                    && matches!(
                        inp.1.format,
                        PixelFormat::B10G11R11Float | PixelFormat::E5B9G9R9Float
                    )
                    && out.1.format == PixelFormat::B10G11R11Float
        );
        let learned = self.input_width != 0;
        if images_valid && let (Some(inp), Some(out)) = (&input, &output) {
            self.input_width = inp.1.width;
            self.input_height = inp.1.height;
            self.output_address = out.0;
        }

        let (Some(_inp), Some(out), Some(tm)) = (input, output, tonemapped) else {
            self.output_replaced = false;
            if (learned || !images_valid) && !self.logged_unexpected {
                self.logged_unexpected = true;
                tracing::warn!(
                    "Ocarina of Time: unexpected network images; sharpening bypass waiting"
                );
            }
            return Ok(false);
        };
        let tm_addr = tm.address();
        let size_ok = if base_output {
            tm.width <= out.1.width && tm.height <= out.1.height
        } else {
            tm.width == out.1.width && tm.height == out.1.height
        };
        let Some(src_info) = ImageInfo::from_render_target(&tm, 0, gob_layout) else {
            self.output_replaced = false;
            return Ok(false);
        };
        if !images_valid || tm_addr == 0 || tm_addr == out.0 || !size_ok {
            self.output_replaced = false;
            return Ok(false);
        }

        let src = BlitSide {
            gpu_va: tm_addr,
            info: src_info,
            rect: [0, 0, tm.width as i32, tm.height as i32],
        };
        let dst = BlitSide {
            gpu_va: out.0,
            info: out.2,
            rect: [0, 0, out.1.width as i32, out.1.height as i32],
        };
        let filter = if base_output {
            vk::Filter::LINEAR
        } else {
            vk::Filter::NEAREST
        };
        let copied = textures.blit(tx, ctx.memory, &src, &dst, filter)?;
        self.output_replaced = copied;
        self.output_pending = false;
        if !self.logged_replaced {
            self.logged_replaced = true;
            tracing::info!(
                shader = format_args!("{native_key:016x}"),
                src_addr = format_args!("{tm_addr:#x}"),
                src_size = format_args!("{}x{}", tm.width, tm.height),
                dst_addr = format_args!("{:#x}", out.0),
                dst_size = format_args!("{}x{}", out.1.width, out.1.height),
                copied,
                "Ocarina of Time: anti-aliasing network bypassed"
            );
        }
        Ok(copied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_same_size_draw_away_from_output() {
        assert!(is_tonemap_candidate(0x2000, 2560, 1440, 2560, 1440, 0x3000));
    }

    #[test]
    fn rejects_other_sizes_and_output_target() {
        assert!(!is_tonemap_candidate(0, 2560, 1440, 2560, 1440, 0x3000));
        assert!(!is_tonemap_candidate(
            0x3000, 2560, 1440, 2560, 1440, 0x3000
        ));
        assert!(!is_tonemap_candidate(0x2000, 1280, 720, 2560, 1440, 0x3000));
        assert!(!is_tonemap_candidate(0x2000, 2560, 1440, 0, 0, 0x3000));
    }

    #[test]
    fn skips_unused_dispatch_only_while_output_bypass_is_pending() {
        assert!(should_skip_unused_dispatch(
            UNUSED_DISPATCH_PROGRAM_KEY,
            true,
            true
        ));
        assert!(!should_skip_unused_dispatch(
            UNUSED_DISPATCH_PROGRAM_KEY,
            true,
            false
        ));
        assert!(!should_skip_unused_dispatch(
            UNUSED_DISPATCH_PROGRAM_KEY,
            false,
            true
        ));
        assert!(!should_skip_unused_dispatch(
            NETWORK_PROGRAM_KEY,
            true,
            true
        ));
    }
}
