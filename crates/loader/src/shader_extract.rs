//! Extracts standalone NVN2 (Switch 2) shader programs from a title's RomFS (`SARC`, `BNSH`,
//! `BAHS` / `sharcb`, and plain Zstd containers) so they can be pre-translated before gameplay.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

use lichii_fs::{DirRef, DirectoryExt, EntryKind, FileRef};

use crate::zstd::{self, Variant};

const MAX_BLOB: usize = 256 << 20;
const MAX_DEPTH: usize = 8;
const KEY_CONTROL_LEN: usize = 0x48;
const CONTROL_MAGIC: [u8; 4] = [0x34, 0x12, 0x76, 0x98];
const BNSH_MAGIC: [u8; 8] = [b'B', b'N', b'S', b'H', 0, 0, 0, 0];
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtractedStage {
    Vertex,
    TessControl,
    TessEval,
    Geometry,
    Fragment,
    Compute,
    Task,
    Mesh,
}

#[derive(Clone, Debug)]
pub struct ExtractedShader {
    pub stage: ExtractedStage,
    pub control: Vec<u8>,
    pub code: Vec<u8>,
    pub fingerprint: u64,
}

fn read_u16_le(data: &[u8], at: usize) -> Option<u16> {
    data.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
}

fn read_u16_be(data: &[u8], at: usize) -> Option<u16> {
    data.get(at..at + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
}

fn read_u32_le(data: &[u8], at: usize) -> Option<u32> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn read_u32_be(data: &[u8], at: usize) -> Option<u32> {
    data.get(at..at + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

fn read_i32_le(data: &[u8], at: usize) -> Option<i32> {
    data.get(at..at + 4)
        .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn read_u64_le(data: &[u8], at: usize) -> Option<u64> {
    data.get(at..at + 8)
        .map(|b| u64::from_le_bytes(b.try_into().expect("8 bytes")))
}

fn is_nvn2_control(control: &[u8]) -> bool {
    control.len() >= KEY_CONTROL_LEN
        && control[..4] == CONTROL_MAGIC
        && control[4] == 0xc9
        && control[5..8] == [0, 0, 0]
}

fn nvn2_stage(control: &[u8], code: &[u8]) -> Option<ExtractedStage> {
    if control.len() > 0x56 {
        match control[0x56] {
            5 => return Some(ExtractedStage::Compute),
            6 => return Some(ExtractedStage::Task),
            7 => return Some(ExtractedStage::Mesh),
            _ => {}
        }
    }
    let word0 = read_u32_le(code, 0)?;
    match (word0 >> 10) & 0xF {
        1 => Some(ExtractedStage::Vertex),
        2 => Some(ExtractedStage::TessControl),
        3 => Some(ExtractedStage::TessEval),
        4 => Some(ExtractedStage::Geometry),
        5 => Some(ExtractedStage::Fragment),
        _ => None,
    }
}

/// 64-bit FNV-1a fingerprint over `control[..0x48]` (with `+0x30..+0x34` zeroed) and `code`.
pub fn fingerprint(control: &[u8], code: &[u8]) -> u64 {
    let mut head = [0u8; KEY_CONTROL_LEN];
    let n = control.len().min(KEY_CONTROL_LEN);
    head[..n].copy_from_slice(&control[..n]);
    head[0x30..0x34].fill(0);
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for &b in head.iter().chain(code.iter()) {
        hash = (hash ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

struct Extractor {
    seen: HashSet<u64>,
    out: Vec<ExtractedShader>,
}

impl Extractor {
    fn new() -> Self {
        Self {
            seen: HashSet::new(),
            out: Vec::new(),
        }
    }

    fn emit(&mut self, stage: ExtractedStage, control: &[u8], code: &[u8]) {
        if !is_nvn2_control(control) || code.is_empty() {
            return;
        }
        let fp = fingerprint(control, code);
        if self.seen.insert(fp) {
            self.out.push(ExtractedShader {
                stage,
                control: control.to_vec(),
                code: code.to_vec(),
                fingerprint: fp,
            });
        }
    }

    fn inspect(&mut self, data: &[u8], depth: usize) {
        if depth >= MAX_DEPTH || data.len() < 8 {
            return;
        }
        if data.starts_with(&ZSTD_MAGIC) {
            if let Ok(decompressed) = decompress_zstd(data) {
                self.inspect(&decompressed, depth + 1);
            }
            return;
        }
        if data.starts_with(b"SARC") {
            self.parse_sarc(data, depth + 1);
            return;
        }
        if data.starts_with(b"BAHS") {
            self.parse_bahs(data);
            return;
        }
        self.scan_embedded_bnsh(data);
    }

    fn parse_sarc(&mut self, data: &[u8], depth: usize) {
        let Some(bom) = read_u16_le(data, 6) else {
            return;
        };
        let big = bom == 0xFFFE;
        let u16_at = |at| {
            if big {
                read_u16_be(data, at)
            } else {
                read_u16_le(data, at)
            }
        };
        let u32_at = |at| {
            if big {
                read_u32_be(data, at)
            } else {
                read_u32_le(data, at)
            }
        };

        let Some(header_len) = u16_at(4).map(usize::from) else {
            return;
        };
        let Some(data_offset) = u32_at(12).map(|v| v as usize) else {
            return;
        };
        if header_len < 0x14 || data_offset > data.len() {
            return;
        }
        let sfat = header_len;
        if data.get(sfat..sfat + 4) != Some(b"SFAT") {
            return;
        }
        let Some(sfat_len) = u16_at(sfat + 4).map(usize::from) else {
            return;
        };
        let Some(count) = u16_at(sfat + 6).map(usize::from) else {
            return;
        };
        let nodes = sfat + sfat_len;
        for i in 0..count {
            let node = nodes + i * 0x10;
            let (Some(start), Some(end)) = (u32_at(node + 8), u32_at(node + 12)) else {
                break;
            };
            if end < start {
                continue;
            }
            let abs_start = data_offset.saturating_add(start as usize);
            let abs_end = data_offset.saturating_add(end as usize);
            if let Some(member) = data.get(abs_start..abs_end) {
                self.inspect(member, depth);
            }
        }
    }

    fn scan_embedded_bnsh(&mut self, data: &[u8]) {
        let mut pos = 0;
        while let Some(rel) = data[pos..]
            .windows(BNSH_MAGIC.len())
            .position(|w| w == BNSH_MAGIC)
        {
            let base = pos + rel;
            let size = read_u32_le(data, base + 0x1c).map_or(0, |s| s as usize);
            if size >= 0x98 && base.saturating_add(size) <= data.len() {
                self.parse_bnsh(&data[base..base + size]);
                pos = base + size;
            } else {
                pos = base + BNSH_MAGIC.len();
            }
        }
    }

    fn parse_bnsh(&mut self, d: &[u8]) {
        if d.get(0x60..0x64) != Some(b"grsc") {
            return;
        }
        let (Some(count), Some(table)) = (read_u32_le(d, 0x7c), read_u64_le(d, 0x80)) else {
            return;
        };
        let slots = [
            ExtractedStage::Vertex,
            ExtractedStage::TessControl,
            ExtractedStage::TessEval,
            ExtractedStage::Geometry,
            ExtractedStage::Fragment,
            ExtractedStage::Compute,
        ];
        for v in 0..count as usize {
            let Some(program) =
                read_u64_le(d, table as usize + 0x40 * v + 0x10).map(|p| p as usize)
            else {
                break;
            };
            if program == 0 {
                continue;
            }
            let mut read_stage = |record: usize, stage: ExtractedStage, optional: bool| {
                let (Some(ctrl_at), Some(code_at), Some(code_size), Some(ctrl_size)) = (
                    read_u64_le(d, record + 0x08).map(|v| v as usize),
                    read_u64_le(d, record + 0x10).map(|v| v as usize),
                    read_u32_le(d, record + 0x18).map(|v| v as usize),
                    read_u32_le(d, record + 0x1c).map(|v| v as usize),
                ) else {
                    return;
                };
                if optional && !(KEY_CONTROL_LEN..0x10000).contains(&ctrl_size) {
                    return;
                }
                let Some(control) = d.get(ctrl_at..ctrl_at.saturating_add(ctrl_size)) else {
                    return;
                };
                let Some(code) = d.get(code_at..code_at.saturating_add(code_size)) else {
                    return;
                };
                if !is_nvn2_control(control) || read_u32_le(control, 0x44) != Some(code_size as u32)
                {
                    return;
                }
                self.emit(stage, control, code);
            };

            for (slot_idx, stage) in slots.iter().copied().enumerate() {
                if let Some(record) = read_u64_le(d, program + 8 + 8 * slot_idx)
                    && record != 0
                {
                    read_stage(record as usize, stage, false);
                }
            }
            for (offset, stage) in [
                (0x40usize, ExtractedStage::Task),
                (0x48, ExtractedStage::Mesh),
            ] {
                if let Some(record) = read_u64_le(d, program + offset)
                    && record != 0
                {
                    read_stage(record as usize, stage, true);
                }
            }
        }
    }

    fn parse_bahs(&mut self, d: &[u8]) {
        let (Some(region_size), Some(region)) = (
            read_u32_le(d, 0x18).map(|v| v as usize),
            read_u32_le(d, 0x1c).map(|v| v as usize),
        ) else {
            return;
        };
        if region.saturating_add(region_size) > d.len() || region < 16 {
            return;
        }
        let mut found = Vec::new();
        for a in (0..region.saturating_sub(15)).step_by(4) {
            let (Some(offset), Some(zero), Some(ctrl_size), Some(distance)) = (
                read_u32_le(d, a).map(|v| v as usize),
                read_u32_le(d, a + 4),
                read_u32_le(d, a + 8).map(|v| v as usize),
                read_i32_le(d, a + 12),
            ) else {
                break;
            };
            if zero != 0
                || !offset.is_multiple_of(0x100)
                || !(KEY_CONTROL_LEN..0x10000).contains(&ctrl_size)
            {
                continue;
            }
            let Some(ctrl_at) = a.checked_add_signed(distance as isize) else {
                continue;
            };
            let Some(control) = d.get(ctrl_at..ctrl_at.saturating_add(ctrl_size)) else {
                continue;
            };
            if ctrl_at + ctrl_size > region || !is_nvn2_control(control) {
                continue;
            }
            let Some(code_size) = read_u32_le(control, 0x44).map(|v| v as usize) else {
                continue;
            };
            if offset.saturating_add(code_size) > region_size {
                continue;
            }
            let Some(code) = d.get(region + offset..region + offset + code_size) else {
                continue;
            };
            let Some(stage) = nvn2_stage(control, code) else {
                continue;
            };
            found.push((a, ctrl_at, stage));
        }

        for &(at, ctrl_at, stage) in &found {
            let inside = found
                .iter()
                .any(|&(_, other_ctrl, _)| other_ctrl <= at && at < other_ctrl + KEY_CONTROL_LEN);
            if inside {
                continue;
            }
            let offset = read_u32_le(d, at).expect("checked") as usize;
            let ctrl_size = read_u32_le(d, at + 8).expect("checked") as usize;
            let control = &d[ctrl_at..ctrl_at + ctrl_size];
            let code_size = read_u32_le(control, 0x44).expect("checked") as usize;
            let code = &d[region + offset..region + offset + code_size];
            self.emit(stage, control, code);
        }
    }
}

fn decompress_zstd(src: &[u8]) -> Result<Vec<u8>, zstd::ZstdError> {
    zstd::decompress_all(Variant::Zstd, src, MAX_BLOB)
}

fn collect_files(dir: &DirRef, out: &mut Vec<FileRef>) {
    let Ok(entries) = dir.entries() else {
        return;
    };
    for entry in entries {
        match entry.kind {
            EntryKind::File => {
                if let Ok(Some(file)) = dir.open_file(&entry.name) {
                    out.push(file);
                }
            }
            EntryKind::Directory => {
                if let Ok(Some(sub)) = dir.open_dir(&entry.name) {
                    collect_files(&sub, out);
                }
            }
        }
    }
}

/// Scans `romfs` for standalone NVN2 shader programs, reporting `(done_files, total_files)`
/// progress and stopping early if `cancel` is set.
pub fn extract_from_romfs(
    romfs: &DirRef,
    cancel: &AtomicBool,
    on_progress: impl Fn(usize, usize),
) -> Vec<ExtractedShader> {
    let mut files = Vec::new();
    collect_files(romfs, &mut files);
    let total = files.len();
    let mut extractor = Extractor::new();
    let mut buf = Vec::new();
    for (i, file) in files.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        on_progress(i, total);
        let size = file.size();
        if size == 0 || size as usize > MAX_BLOB {
            continue;
        }
        buf.resize(size as usize, 0);
        if file.read_at(0, &mut buf).is_ok() {
            extractor.inspect(&buf, 0);
        }
    }
    on_progress(total, total);
    extractor.out
}

#[cfg(test)]
mod tests {
    use super::*;
    use lichii_fs::MemDir;

    fn fake_nvn2_compute_control(code_len: u32) -> Vec<u8> {
        let mut ctrl = vec![0u8; 0x80];
        ctrl[..4].copy_from_slice(&CONTROL_MAGIC);
        ctrl[4] = 0xc9;
        ctrl[0x44..0x48].copy_from_slice(&code_len.to_le_bytes());
        ctrl[0x56] = 5; // Compute
        ctrl
    }

    fn build_bnsh_with_compute(control: &[u8], code: &[u8]) -> Vec<u8> {
        let total = 0x200 + control.len() + code.len();
        let mut bnsh = vec![0u8; total];
        bnsh[..8].copy_from_slice(&BNSH_MAGIC);
        bnsh[0x1c..0x20].copy_from_slice(&(total as u32).to_le_bytes());
        bnsh[0x60..0x64].copy_from_slice(b"grsc");
        bnsh[0x7c..0x80].copy_from_slice(&1u32.to_le_bytes()); // 1 variation
        bnsh[0x80..0x88].copy_from_slice(&0x90u64.to_le_bytes()); // table at 0x90
        bnsh[0x90 + 0x10..0x90 + 0x18].copy_from_slice(&0xE0u64.to_le_bytes()); // program at 0xE0
        // slot 5 (Compute) record at 0x140
        bnsh[0xE0 + 8 + 8 * 5..0xE0 + 8 + 8 * 6].copy_from_slice(&0x140u64.to_le_bytes());
        let ctrl_at = 0x200usize;
        let code_at = ctrl_at + control.len();
        bnsh[0x140 + 0x08..0x140 + 0x10].copy_from_slice(&(ctrl_at as u64).to_le_bytes());
        bnsh[0x140 + 0x10..0x140 + 0x18].copy_from_slice(&(code_at as u64).to_le_bytes());
        bnsh[0x140 + 0x18..0x140 + 0x1c].copy_from_slice(&(code.len() as u32).to_le_bytes());
        bnsh[0x140 + 0x1c..0x140 + 0x20].copy_from_slice(&(control.len() as u32).to_le_bytes());
        bnsh[ctrl_at..code_at].copy_from_slice(control);
        bnsh[code_at..code_at + code.len()].copy_from_slice(code);
        bnsh
    }

    #[test]
    fn extracts_deduplicated_bnsh_compute_shader_from_romfs() {
        let code = vec![0xAAu8; 64];
        let control = fake_nvn2_compute_control(code.len() as u32);
        let bnsh = build_bnsh_with_compute(&control, &code);
        let dir: DirRef = std::sync::Arc::new(
            MemDir::new()
                .with_bytes("a.bnsh", bnsh.clone())
                .with_bytes("b.bnsh", bnsh),
        );
        let cancel = AtomicBool::new(false);
        let extracted = extract_from_romfs(&dir, &cancel, |_, _| {});
        assert_eq!(extracted.len(), 1);
        assert_eq!(extracted[0].stage, ExtractedStage::Compute);
        assert_eq!(extracted[0].code, code);
    }
}
