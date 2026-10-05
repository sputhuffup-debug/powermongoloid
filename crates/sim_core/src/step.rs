use bevy_app::{App, FixedMain};
use bevy_ecs::message::message_update_system;
use bevy_ecs::prelude::*;

/// Runs exactly one simulation tick with no dependence on wall-clock time,
/// then rotates message buffers. Used by `sim_headless` and tests; the game
/// instead lets Bevy's fixed-timestep driver run `FixedMain`.
pub fn run_sim_tick(world: &mut World) {
    world.run_schedule(FixedMain);
    // Without bevy_time, nothing else swaps message double-buffers; do it once
    // per tick so messages live for exactly two ticks, as in the game.
    world
        .run_system_cached(message_update_system)
        .expect("message_update_system is a valid system");
}

/// Extension for driving an [`App`] tick by tick.
pub trait SimStepper {
    /// Finishes plugin setup. Call once before [`SimStepper::step`].
    fn prepare_sim(&mut self) -> &mut Self;
    /// Runs `n` ticks.
    fn step(&mut self, n: u64) -> &mut Self;
}

impl SimStepper for App {
    fn prepare_sim(&mut self) -> &mut Self {
        self.finish();
        self.cleanup();
        self
    }

    fn step(&mut self, n: u64) -> &mut Self {
        for _ in 0..n {
            run_sim_tick(self.world_mut());
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommandKind, CommandsProcessed, FactionId, SimCommand, SimCorePlugin, SimTick};

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(SimCorePlugin { seed: 1 }).prepare_sim();
        app
    }

    #[test]
    fn tick_advances_once_per_step() {
        let mut app = app();
        app.step(25);
        assert_eq!(app.world().resource::<SimTick>().0, 25);
    }

    #[test]
    fn commands_are_applied_exactly_once() {
        let mut app = app();
        app.world_mut().write_message(SimCommand {
            issuer: FactionId(0),
            kind: CommandKind::Noop,
        });
        app.step(5);
        assert_eq!(app.world().resource::<CommandsProcessed>().0, 1);
    }
}
