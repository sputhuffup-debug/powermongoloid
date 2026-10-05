//! UI-07: tick time per system set, entity counts, LOD state, RNG seed.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use sim_core::{DetailLevel, SimSeed, SimSet, SimTick, SimTimings, SpawnId};

pub struct DebugPanelPlugin;

impl Plugin for DebugPanelPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }
        app.add_plugins(EguiPlugin::default())
            .add_systems(EguiPrimaryContextPass, debug_panel);
    }
}

fn ms(d: std::time::Duration) -> String {
    format!("{:.3} ms", d.as_secs_f64() * 1000.0)
}

fn debug_panel(
    mut contexts: EguiContexts,
    tick: Res<SimTick>,
    seed: Res<SimSeed>,
    timings: Res<SimTimings>,
    fixed: Res<Time<Fixed>>,
    diagnostics: Res<DiagnosticsStore>,
    sim_entities: Query<Option<&DetailLevel>, With<SpawnId>>,
) -> Result {
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);
    let (mut full, mut dormant, mut untagged) = (0usize, 0usize, 0usize);
    for detail in &sim_entities {
        match detail {
            Some(d) if d.full => full += 1,
            Some(_) => dormant += 1,
            None => untagged += 1,
        }
    }

    egui::Window::new("Sim debug")
        .default_pos([12.0, 12.0])
        .resizable(false)
        .show(contexts.ctx_mut()?, |ui| {
            egui::Grid::new("summary").num_columns(2).show(ui, |ui| {
                ui.label("Tick");
                ui.monospace(tick.0.to_string());
                ui.end_row();
                ui.label("Tick rate");
                ui.monospace(format!("{:.0} Hz", 1.0 / fixed.timestep().as_secs_f64()));
                ui.end_row();
                ui.label("Tick time");
                ui.monospace(format!(
                    "{} (avg {})",
                    ms(timings.total),
                    ms(timings.total_avg)
                ));
                ui.end_row();
                ui.label("Frame rate");
                ui.monospace(format!("{fps:.0} fps"));
                ui.end_row();
                ui.label("Seed");
                ui.monospace(seed.0.to_string());
                ui.end_row();
            });

            ui.separator();
            ui.label("Tick time by system set");
            egui::Grid::new("sets")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    for (set, d) in SimSet::ALL.iter().zip(timings.per_set) {
                        ui.label(set.name());
                        ui.monospace(ms(d));
                        ui.end_row();
                    }
                });

            ui.separator();
            ui.label("Entities by detail level");
            egui::Grid::new("detail").num_columns(2).show(ui, |ui| {
                ui.label("Full");
                ui.monospace(full.to_string());
                ui.end_row();
                ui.label("Dormant");
                ui.monospace(dormant.to_string());
                ui.end_row();
                ui.label("Untagged (M0 placeholders)");
                ui.monospace(untagged.to_string());
                ui.end_row();
            });

            ui.separator();
            ui.label("Region LOD: not yet simulated (M5)");
        });
    Ok(())
}
