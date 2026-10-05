use bevy_ecs::prelude::*;

use crate::components::FactionId;
use crate::hash::{StateHash, StateHasher};

/// The only way player and AI input reach the simulation. A replay is just a
/// recorded stream of these.
#[derive(Message, Debug, Clone, PartialEq)]
pub struct SimCommand {
    pub issuer: FactionId,
    pub kind: CommandKind,
}

/// Command payloads. Real variants (`IssueOrder`, ...) arrive in M3 (GP-02).
#[derive(Debug, Clone, PartialEq)]
pub enum CommandKind {
    /// Does nothing; lets M0 exercise the command path end to end.
    Noop,
}

/// Number of commands applied so far. Part of the state hash, so a dropped or
/// duplicated command changes the hash.
#[derive(Resource, Debug, Default)]
pub struct CommandsProcessed(pub u64);

impl StateHash for CommandsProcessed {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.0);
    }
}

pub(crate) fn drain_commands(
    mut commands: MessageReader<SimCommand>,
    mut processed: ResMut<CommandsProcessed>,
) {
    for command in commands.read() {
        match command.kind {
            CommandKind::Noop => {}
        }
        processed.0 += 1;
    }
}
