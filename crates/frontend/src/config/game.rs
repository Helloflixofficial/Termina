//! The per-game Properties window: Add-Ons and settings a game overrides, each with a Custom
//! toggle; the rest follow the global configuration.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use egui::{RichText, Ui};
use lichii_common::settings::{ScalingFilter, Settings};
use lichii_loader::{AddonEntry, ModOptions, encode_disabled_addons, list_addons};

use super::{combo, docked_toggle, note, section, speed_limit, time_zone};
use crate::per_game::{self, GameConfig, copy_key};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    AddOns,
    System,
    Cpu,
    Graphics,
    Audio,
}

impl Page {
    const ALL: [Self; 5] = [
        Self::AddOns,
        Self::System,
        Self::Cpu,
        Self::Graphics,
        Self::Audio,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::AddOns => "Add-Ons",
            Self::System => "System",
            Self::Cpu => "CPU",
            Self::Graphics => "Graphics",
            Self::Audio => "Audio",
        }
    }
}

pub struct GameConfigWindow {
    pub title: String,
    pub program_id: u64,
    pub config: GameConfig,
    /// The settings the game would run with: global, with the custom ones edited.
    draft: Settings,
    custom: BTreeSet<&'static str>,
    addons: Vec<AddonEntry>,
    shader_cache: Option<PathBuf>,
    shader_cache_bytes: u64,
    shader_cache_status: Option<String>,
    page: Page,
}

#[derive(Default)]
pub struct GameConfigOutput {
    /// The configuration file was written; a running game picks it up.
    pub saved: bool,
    pub close: bool,
}

impl GameConfigWindow {
    pub fn new(
        title: String,
        program_id: u64,
        config: GameConfig,
        global: &Settings,
        mod_options: &ModOptions,
        shader_cache: Option<PathBuf>,
    ) -> Self {
        let mut opts = mod_options.clone();
        opts.disabled = config.disabled_addons();
        let addons = if program_id != 0 {
            list_addons(&opts, program_id).unwrap_or_default()
        } else {
            Vec::new()
        };
        let shader_cache_bytes = shader_cache.as_deref().map_or(0, dir_size_bytes);
        Self {
            title,
            program_id,
            draft: config.apply(global),
            custom: config.overridden(),
            config,
            addons,
            shader_cache,
            shader_cache_bytes,
            shader_cache_status: None,
            page: Page::AddOns,
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        global: &Settings,
        running: bool,
    ) -> GameConfigOutput {
        let mut out = GameConfigOutput::default();
        let mut open = true;
        egui::Window::new(format!("Properties: {}", self.title))
            .open(&mut open)
            .default_size([740.0, 560.0])
            .collapsible(false)
            .show(ctx, |ui| {
                egui::Panel::bottom("game config buttons").show(ui, |ui| {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() {
                            out.saved = self.save();
                            out.close = true;
                        }
                        if ui.button("Apply").clicked() {
                            out.saved = self.save();
                        }
                        if ui.button("Cancel").clicked() {
                            out.close = true;
                        }
                        ui.separator();
                        if ui
                            .add_enabled(!self.custom.is_empty(), egui::Button::new("Use Global for All"))
                            .clicked()
                        {
                            self.custom.clear();
                            self.draft = global.clone();
                        }
                    });
                });
                egui::CentralPanel::default().show(ui, |ui| {
                    let id = match self.program_id {
                        0 => "no title ID".to_owned(),
                        id => format!("title ID {id:016X}"),
                    };
                    let file = self.config.file().display().to_string();
                    ui.label(RichText::new(format!("{id} \u{b7} {file}")).weak().small());
                    ui.horizontal(|ui| {
                        for page in Page::ALL {
                            ui.selectable_value(&mut self.page, page, page.label());
                        }
                    });
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .auto_shrink(false)
                        .show(ui, |ui| self.page_ui(ui, global, running));
                    if running {
                        note(
                            ui,
                            if self.page == Page::AddOns {
                                "Restart the game to apply Add-On changes."
                            } else {
                                "Graphics changes reach the running game on Apply; the rest apply the next time it starts."
                            },
                        );
                    }
                });
            });
        if !open {
            out.close = true;
        }
        out
    }

    /// Follows a change made outside this window (the status bar, a hotkey, Configure):
    /// settings whose value for this game went from `before` to `after` take the new value, and
    /// the window's unsaved edits to other settings stay.
    pub fn follow(&mut self, before: &Settings, after: &Settings) {
        per_game::copy_changes(before, after, &mut self.draft);
    }

    fn save(&mut self) -> bool {
        self.config.set_overrides(&self.draft, &self.custom);
        self.config
            .set_disabled_addons(encode_disabled_addons(&self.addons));
        match self.config.save() {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!("{}: {e}", self.config.file().display());
                false
            }
        }
    }

    fn clear_shader_cache(&mut self) {
        let Some(dir) = &self.shader_cache else {
            return;
        };
        match std::fs::remove_dir_all(dir) {
            Ok(()) => {
                self.shader_cache_bytes = 0;
                self.shader_cache_status = Some("Cleared shader cache.".into());
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.shader_cache_bytes = 0;
                self.shader_cache_status = Some("Shader cache was already empty.".into());
            }
            Err(e) => {
                self.shader_cache_bytes = dir_size_bytes(dir);
                self.shader_cache_status = Some(format!("Could not clear shader cache: {e}"));
            }
        }
    }

    fn page_ui(&mut self, ui: &mut Ui, global: &Settings, running: bool) {
        if self.page == Page::AddOns {
            section(ui, "Add-Ons and Mods");
            if self.addons.is_empty() {
                note(ui, "No mods or built-in add-ons found for this title.");
                return;
            }
            egui::Grid::new("game addons")
                .num_columns(2)
                .spacing([24.0, 6.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(RichText::new("Patch Name").strong());
                    ui.label(RichText::new("Type").strong());
                    ui.end_row();
                    for addon in &mut self.addons {
                        let cb = ui.checkbox(&mut addon.enabled, &addon.name);
                        let kind = ui.label(RichText::new(&addon.kind).weak());
                        if !addon.description.is_empty() {
                            cb.on_hover_text(&addon.description);
                            kind.on_hover_text(&addon.description);
                        }
                        ui.end_row();
                    }
                });
            return;
        }

        let mut rows = Rows {
            custom: &mut self.custom,
            draft: &mut self.draft,
            global,
        };
        match self.page {
            Page::AddOns => unreachable!(),
            Page::System => {
                section(ui, "Console");
                rows.grid(ui, "game system", |ui, r| {
                    r.row(
                        ui,
                        "system.preferred_console",
                        "Preferred console",
                        |ui, d| {
                            combo(ui, "game console", &mut d.system.preferred_console);
                        },
                    );
                    r.row(ui, "system.docked", "Console mode", |ui, d| {
                        docked_toggle(ui, &mut d.system.docked);
                    });
                    r.row(ui, "system.language", "Language", |ui, d| {
                        combo(ui, "game language", &mut d.system.language);
                    });
                    r.row(ui, "system.region", "Region", |ui, d| {
                        combo(ui, "game region", &mut d.system.region);
                    });
                    r.row(ui, "system.memory_layout", "Memory layout", |ui, d| {
                        combo(ui, "game memory", &mut d.system.memory_layout);
                    });
                    r.row(ui, "system.time_zone", "Time zone", |ui, d| {
                        time_zone(ui, &mut d.system.time_zone);
                    });
                    r.row(ui, "system.network_enabled", "Network", |ui, d| {
                        ui.checkbox(&mut d.system.network_enabled, "Allow the game to go online");
                    });
                    r.row(
                        ui,
                        "system.builtin_game_patches",
                        "Built-in patches",
                        |ui, d| {
                            ui.checkbox(
                                &mut d.system.builtin_game_patches,
                                "Apply Lichii's built-in patches for known builds",
                            );
                        },
                    );
                });
            }
            Page::Cpu => {
                section(ui, "CPU");
                rows.grid(ui, "game cpu", |ui, r| {
                    r.row(ui, "cpu.backend", "Backend", |ui, d| {
                        combo(ui, "game cpu backend", &mut d.cpu.backend);
                    });
                    r.row(ui, "cpu.accuracy", "Accuracy", |ui, d| {
                        combo(ui, "game cpu accuracy", &mut d.cpu.accuracy);
                    });
                    r.row(ui, "cpu.fastmem", "Fastmem", |ui, d| {
                        ui.checkbox(&mut d.cpu.fastmem, "Map guest memory into the host");
                    });
                });
            }
            Page::Graphics => {
                section(ui, "Image");
                rows.grid(ui, "game graphics", |ui, r| {
                    r.row(ui, "graphics.resolution_scale", "Resolution", |ui, d| {
                        combo(ui, "game resolution", &mut d.graphics.resolution_scale);
                    });
                    r.row(ui, "graphics.scaling_filter", "Scaling filter", |ui, d| {
                        combo(ui, "game scaling filter", &mut d.graphics.scaling_filter);
                    });
                    r.row(ui, "graphics.fsr_sharpness", "FSR sharpness", |ui, d| {
                        ui.add_enabled(
                            d.graphics.scaling_filter == ScalingFilter::Fsr,
                            egui::Slider::new(&mut d.graphics.fsr_sharpness, 0..=100).suffix("%"),
                        );
                    });
                    r.row(ui, "graphics.anti_aliasing", "Anti-aliasing", |ui, d| {
                        combo(ui, "game aa", &mut d.graphics.anti_aliasing);
                    });
                    r.row(
                        ui,
                        "graphics.oot_anti_aliasing",
                        "Ocarina of Time AA",
                        |ui, d| {
                            combo(ui, "game oot aa", &mut d.graphics.oot_anti_aliasing);
                        },
                    );
                    r.row(ui, "graphics.aspect_ratio", "Aspect ratio", |ui, d| {
                        combo(ui, "game aspect", &mut d.graphics.aspect_ratio);
                    });
                    r.row(ui, "graphics.present_mode", "VSync mode", |ui, d| {
                        combo(ui, "game present mode", &mut d.graphics.present_mode);
                    });
                    r.row(ui, "graphics.speed_limit", "Speed", |ui, d| {
                        speed_limit(ui, &mut d.graphics.speed_limit);
                    });
                });
                section(ui, "Accuracy and textures");
                rows.grid(ui, "game advanced", |ui, r| {
                    r.row(ui, "graphics.gpu_accuracy", "GPU accuracy", |ui, d| {
                        combo(ui, "game gpu accuracy", &mut d.graphics.gpu_accuracy);
                    });
                    r.row(
                        ui,
                        "graphics.anisotropic_filtering",
                        "Anisotropic filtering",
                        |ui, d| {
                            combo(ui, "game aniso", &mut d.graphics.anisotropic_filtering);
                        },
                    );
                    r.row(ui, "graphics.fast_gpu_time", "GPU clocks", |ui, d| {
                        combo(ui, "game gpu clocks", &mut d.graphics.fast_gpu_time);
                    });
                    r.row(ui, "graphics.astc_decode", "ASTC decoding", |ui, d| {
                        combo(ui, "game astc decode", &mut d.graphics.astc_decode);
                    });
                    r.row(
                        ui,
                        "graphics.astc_recompression",
                        "ASTC recompression",
                        |ui, d| {
                            combo(
                                ui,
                                "game astc recompression",
                                &mut d.graphics.astc_recompression,
                            );
                        },
                    );
                    r.row(ui, "graphics.vram_usage", "VRAM usage", |ui, d| {
                        combo(ui, "game vram", &mut d.graphics.vram_usage);
                    });
                    r.row(
                        ui,
                        "graphics.async_shader_compilation",
                        "Shaders",
                        |ui, d| {
                            ui.checkbox(
                                &mut d.graphics.async_shader_compilation,
                                "Build shaders asynchronously",
                            );
                        },
                    );
                    r.row(ui, "graphics.disk_shader_cache", "Shader cache", |ui, d| {
                        ui.checkbox(
                            &mut d.graphics.disk_shader_cache,
                            "Use the disk shader cache",
                        );
                    });
                });
                if self.shader_cache.is_some() {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "Cached on disk: {}",
                            format_bytes(self.shader_cache_bytes)
                        ));
                        let clear_btn = ui.add_enabled(
                            !running && self.shader_cache_bytes > 0,
                            egui::Button::new("Clear Shader Cache"),
                        );
                        let clear_btn = if running {
                            clear_btn.on_hover_text("Stop the running game before clearing its shader cache.")
                        } else {
                            clear_btn.on_hover_text("Remove compiled shaders and pipelines cached on disk for this title.")
                        };
                        if clear_btn.clicked() {
                            self.clear_shader_cache();
                        }
                    });
                    if let Some(status) = &self.shader_cache_status {
                        note(ui, status);
                    }
                }
            }
            Page::Audio => {
                section(ui, "Audio");
                rows.grid(ui, "game audio", |ui, r| {
                    r.row(ui, "audio.volume", "Volume", |ui, d| {
                        ui.add(egui::Slider::new(&mut d.audio.volume, 0..=200).suffix("%"));
                    });
                    r.row(ui, "audio.muted", "Mute", |ui, d| {
                        ui.checkbox(&mut d.audio.muted, "Mute this game");
                    });
                });
            }
        }
    }
}

/// Rows of overridable settings: a label, a Custom toggle, and the setting, editable only while
/// it is custom. Turning Custom off shows the global value again.
struct Rows<'a> {
    custom: &'a mut BTreeSet<&'static str>,
    draft: &'a mut Settings,
    global: &'a Settings,
}

impl Rows<'_> {
    fn grid(&mut self, ui: &mut Ui, id: &str, add: impl FnOnce(&mut Ui, &mut Self)) {
        egui::Grid::new(id)
            .num_columns(3)
            .spacing([16.0, 8.0])
            .min_col_width(80.0)
            .show(ui, |ui| add(ui, self));
    }

    fn row(
        &mut self,
        ui: &mut Ui,
        key: &'static str,
        label: &str,
        add: impl FnOnce(&mut Ui, &mut Settings),
    ) {
        ui.horizontal(|ui| {
            ui.set_min_width(150.0);
            ui.label(label);
        });
        let mut on = self.custom.contains(key);
        let toggle = ui
            .checkbox(&mut on, "Custom")
            .on_hover_text("Off: use the global setting");
        if toggle.changed() {
            if on {
                self.custom.insert(key);
            } else {
                self.custom.remove(key);
                copy_key(self.global, self.draft, key);
            }
        }
        ui.add_enabled_ui(on, |ui| ui.horizontal(|ui| add(ui, self.draft)));
        ui.end_row();
    }
}

fn dir_size_bytes(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|entry| Some((entry.path(), entry.metadata().ok()?)))
        .map(|(entry_path, meta)| {
            if meta.is_dir() {
                dir_size_bytes(&entry_path)
            } else {
                meta.len()
            }
        })
        .sum()
}

fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{} KiB", bytes / KIB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lichii_common::settings::{CpuAccuracy, ResolutionScale};

    #[test]
    fn outside_changes_follow_into_the_draft_without_undoing_its_edits() {
        let global = Settings::default();
        let mut window = GameConfigWindow::new(
            "Game".into(),
            1,
            GameConfig::default(),
            &global,
            &ModOptions::default(),
            None,
        );
        window.custom.insert("cpu.accuracy");
        window.draft.cpu.accuracy = CpuAccuracy::Paranoid;

        let mut after = global.clone();
        after.graphics.resolution_scale = ResolutionScale::X2;
        window.follow(&global, &after);

        assert_eq!(window.draft.graphics.resolution_scale, ResolutionScale::X2);
        assert_eq!(window.draft.cpu.accuracy, CpuAccuracy::Paranoid);
    }

    #[test]
    fn clear_shader_cache_removes_files_and_updates_size() {
        let dir = std::env::temp_dir().join(format!("lichii-gamecfg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("shaders.bin"), vec![1u8; 4096]).unwrap();
        std::fs::write(dir.join("pipelines.bin"), vec![2u8; 2048]).unwrap();

        let global = Settings::default();
        let mut window = GameConfigWindow::new(
            "Game".into(),
            1,
            GameConfig::default(),
            &global,
            &ModOptions::default(),
            Some(dir.clone()),
        );
        assert_eq!(window.shader_cache_bytes, 6144);
        window.clear_shader_cache();
        assert_eq!(window.shader_cache_bytes, 0);
        assert!(!dir.exists());
        assert_eq!(
            window.shader_cache_status.as_deref(),
            Some("Cleared shader cache.")
        );
    }
}
