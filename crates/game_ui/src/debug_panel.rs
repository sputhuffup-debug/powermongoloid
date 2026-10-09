//! UI-07: tick time per system set, entity counts, LOD state, RNG seed.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use game_render::{CameraRig, TerrainChunks};
use sim_core::{DetailLevel, SimSeed, SimSet, SimTick, SimTimings, SpawnId};
use sim_world::{RoadNetwork, Settlement, WorldGenReport, WorldMap};

pub struct DebugPanelPlugin;

impl Plugin for DebugPanelPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }
        app.add_plugins(EguiPlugin::default())
            .add_systems(EguiPrimaryContextPass, debug_panel.run_if(crate::playing));
    }
}

fn ms(d: std::time::Duration) -> String {
    format!("{:.3} ms", d.as_secs_f64() * 1000.0)
}

/// World and render state shown in the panel.
#[derive(bevy::ecs::system::SystemParam)]
struct WorldInfo<'w, 's> {
    map: Option<Res<'w, WorldMap>>,
    report: Option<Res<'w, WorldGenReport>>,
    roads: Option<Res<'w, RoadNetwork>>,
    chunks: Option<Res<'w, TerrainChunks>>,
    agents: Option<Res<'w, game_render::AgentVisuals>>,
    settlements: Query<'w, 's, (), With<Settlement>>,
    rig: Query<'w, 's, &'static CameraRig>,
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn debug_panel(
    mut contexts: EguiContexts,
    world: WorldInfo,
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
                ui.label("No detail level (settlements, buildings)");
                ui.monospace(untagged.to_string());
                ui.end_row();
            });

            ui.separator();
            ui.label("World");
            egui::Grid::new("world").num_columns(2).show(ui, |ui| {
                if let Some(map) = &world.map {
                    ui.label("Size");
                    ui.monospace(format!("{0} x {0} tiles", map.size()));
                    ui.end_row();
                }
                ui.label("Settlements");
                ui.monospace(world.settlements.iter().count().to_string());
                ui.end_row();
                if let Some(roads) = &world.roads {
                    ui.label("Roads");
                    ui.monospace(roads.roads.len().to_string());
                    ui.end_row();
                }
                if let Some(report) = &world.report {
                    ui.label("Generated in");
                    ui.monospace(ms(report.total));
                    ui.end_row();
                }
                if let Some(chunks) = &world.chunks {
                    ui.label("Terrain chunks");
                    ui.monospace(format!(
                        "{}/{} built, {} builds",
                        chunks.built_count, chunks.total, chunks.builds
                    ));
                    ui.end_row();
                }
                if let Some(chunks) = &world.chunks {
                    ui.label("Textured chunks");
                    ui.monospace(format!("{} (detail band)", chunks.textured_count));
                    ui.end_row();
                }
                if let Some(agents) = &world.agents {
                    ui.label("People by band");
                    ui.monospace(format!(
                        "tex {} · flat {} · board {} · dot {}",
                        agents.per_band[0],
                        agents.per_band[1],
                        agents.per_band[2],
                        agents.per_band[3]
                    ));
                    ui.end_row();
                }
                if let Ok(rig) = world.rig.single() {
                    ui.label("Camera");
                    ui.monospace(format!(
                        "({:.0}, {:.0}) dist {:.0} yaw {:.0}°",
                        rig.focus.x,
                        rig.focus.y,
                        rig.distance,
                        rig.yaw.to_degrees()
                    ));
                    ui.end_row();
                }
            });

            ui.separator();
            ui.label("Region LOD: not yet simulated (M5)");
        });
    Ok(())
}
