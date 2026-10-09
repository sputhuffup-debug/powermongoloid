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
    use crate::{
        Cause, Chronicle, CommandKind, CommandsProcessed, FactionId, RunConfig, RunOutcome,
        RunState, SimCommand, SimCorePlugin, SimEventKind, SimTick, compute_state_hash,
    };

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(SimCorePlugin { seed: 1, run: None })
            .prepare_sim();
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
        app.world_mut()
            .write_message(SimCommand::player(FactionId(0), CommandKind::Noop));
        app.step(5);
        assert_eq!(app.world().resource::<CommandsProcessed>().0, 1);
    }

    /// GP-21: the tick a run ends on completes; after it the clock and the
    /// state hash freeze, and input is ignored.
    #[test]
    fn a_finished_run_stops_the_clock() {
        let mut app = App::new();
        app.add_plugins(SimCorePlugin {
            seed: 1,
            run: Some(RunConfig::default()),
        })
        .prepare_sim();
        app.step(10);
        let finished = app
            .world_mut()
            .resource_mut::<RunState>()
            .finish(RunOutcome::Abandoned, Cause::Player);
        assert!(finished);
        app.step(1);

        let state = *app.world().resource::<RunState>();
        assert_eq!(state.outcome, Some(RunOutcome::Abandoned));
        assert_eq!(state.cause, Cause::Player);
        assert_eq!(state.ended_at, Some(10));
        assert_eq!(state.pending, None);
        assert_eq!(
            app.world().resource::<SimTick>().0,
            11,
            "the ending tick completes"
        );
        let last = *app
            .world()
            .resource::<Chronicle>()
            .entries()
            .last()
            .unwrap();
        assert_eq!((last.tick, last.event.kind), (10, SimEventKind::RunEnded));
        assert_eq!(last.event.subject, RunOutcome::Abandoned as u64);

        let frozen = compute_state_hash(app.world_mut());
        app.world_mut()
            .write_message(SimCommand::player(FactionId::PLAYER, CommandKind::Noop));
        app.step(50);
        assert_eq!(app.world().resource::<SimTick>().0, 11);
        assert_eq!(app.world().resource::<CommandsProcessed>().0, 0);
        assert_eq!(compute_state_hash(app.world_mut()), frozen);
        assert!(
            !app.world_mut()
                .resource_mut::<RunState>()
                .finish(RunOutcome::Won, Cause::None),
            "a finished run can't end again"
        );
    }

    #[test]
    fn outside_a_run_the_clock_never_stops() {
        let mut app = app();
        let finished = app
            .world_mut()
            .resource_mut::<RunState>()
            .finish(RunOutcome::Lost, Cause::None);
        assert!(!finished);
        app.step(5);
        assert_eq!(app.world().resource::<SimTick>().0, 5);
        assert_eq!(app.world().resource::<RunState>().outcome, None);
    }
}
