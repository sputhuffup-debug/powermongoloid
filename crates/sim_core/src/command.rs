//! The command path: the only way player and AI input reach the simulation,
//! plus recording and replay of the command stream (NFR-08, ADR 0004).

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::FactionId;
use crate::hash::{StateHash, StateHasher};
use crate::schedule::SimTick;

/// A player or AI command. A replay is just a recorded stream of these.
#[derive(Message, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimCommand {
    pub issuer: FactionId,
    pub kind: CommandKind,
}

/// Command payloads. Real variants (`IssueOrder`, ...) arrive in M3 (GP-02).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommandKind {
    /// Does nothing; lets the harness exercise the command path end to end.
    Noop,
}

/// Ordering inside [`SimSet::Input`](crate::SimSet::Input): scripted commands
/// are injected and recorded first, then consumers read them.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimInputSet {
    /// Replays scripted commands and records every command for this tick.
    Inject,
    /// Systems that act on `SimCommand`s go here.
    Consume,
}

/// A command applied at a given tick.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoggedCommand {
    pub tick: u64,
    pub command: SimCommand,
}

/// Commands scheduled for replay, keyed by the tick they apply on. Each tick,
/// the matching entries are written as `SimCommand` messages, so consumers
/// can't tell a replayed command from a live one.
#[derive(Resource, Debug, Default, Clone)]
pub struct CommandScript {
    by_tick: BTreeMap<u64, Vec<SimCommand>>,
}

impl CommandScript {
    pub fn push(&mut self, tick: u64, command: SimCommand) {
        self.by_tick.entry(tick).or_default().push(command);
    }

    pub fn from_log(log: &[LoggedCommand]) -> Self {
        let mut script = Self::default();
        for entry in log {
            script.push(entry.tick, entry.command.clone());
        }
        script
    }

    pub fn len(&self) -> usize {
        self.by_tick.values().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.by_tick.is_empty()
    }
}

/// Every command applied so far, in application order. Not part of the state
/// hash: it is history, and its effects are already in the hashed state.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct CommandLog(pub Vec<LoggedCommand>);

/// Number of commands applied so far. Part of the state hash, so a dropped or
/// duplicated command changes the hash.
#[derive(Resource, Debug, Default)]
pub struct CommandsProcessed(pub u64);

impl StateHash for CommandsProcessed {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.0);
    }
}

pub(crate) fn inject_scripted(
    tick: Res<SimTick>,
    mut script: ResMut<CommandScript>,
    mut out: MessageWriter<SimCommand>,
) {
    if let Some(commands) = script.by_tick.remove(&tick.0) {
        out.write_batch(commands);
    }
}

pub(crate) fn record_commands(
    tick: Res<SimTick>,
    mut commands: MessageReader<SimCommand>,
    mut log: ResMut<CommandLog>,
) {
    for command in commands.read() {
        log.0.push(LoggedCommand {
            tick: tick.0,
            command: command.clone(),
        });
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
