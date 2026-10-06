//! Replay files: seed + recorded command stream + expected state hashes
//! (NFR-08, ADR 0004). Stored as RON so a divergence can be read and edited.

use serde::{Deserialize, Serialize};

use crate::command::LoggedCommand;

/// Bump when the replay format or the meaning of a command changes.
pub const REPLAY_FORMAT: u32 = 2;

/// The state hash at a given tick (after `tick` ticks have run).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub tick: u64,
    pub hash: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Replay {
    pub format: u32,
    pub seed: u64,
    /// M0 placeholder wanderer count; goes away with the demo in M2.
    pub demo_agents: u32,
    /// Map size in tiles (worldgen settings otherwise come from the build).
    pub world_size: u32,
    pub ticks: u64,
    /// Ticks between checkpoints (0 = only the final state).
    pub checkpoint_every: u64,
    pub commands: Vec<LoggedCommand>,
    /// Expected hashes, in tick order. The last one is the final state.
    pub checkpoints: Vec<Checkpoint>,
}

#[derive(Debug)]
pub enum ReplayError {
    Parse(String),
    Format { found: u32, expected: u32 },
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::Parse(e) => write!(f, "invalid replay file: {e}"),
            ReplayError::Format { found, expected } => {
                write!(
                    f,
                    "replay format {found} is not supported (expected {expected})"
                )
            }
        }
    }
}

impl std::error::Error for ReplayError {}

impl Replay {
    pub fn to_ron(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .expect("replay types always serialize")
    }

    pub fn from_ron(text: &str) -> Result<Self, ReplayError> {
        let replay: Replay = ron::from_str(text).map_err(|e| ReplayError::Parse(e.to_string()))?;
        if replay.format != REPLAY_FORMAT {
            return Err(ReplayError::Format {
                found: replay.format,
                expected: REPLAY_FORMAT,
            });
        }
        Ok(replay)
    }

    /// The first checkpoint where `actual` differs from this replay's
    /// expectations, as `(expected, actual)`.
    pub fn first_divergence(
        &self,
        actual: &[Checkpoint],
    ) -> Option<(Checkpoint, Option<Checkpoint>)> {
        self.checkpoints
            .iter()
            .enumerate()
            .find(|(i, expected)| actual.get(*i) != Some(expected))
            .map(|(i, expected)| (*expected, actual.get(i).copied()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommandKind, FactionId, SimCommand};

    fn sample() -> Replay {
        Replay {
            format: REPLAY_FORMAT,
            seed: 42,
            demo_agents: 8,
            world_size: 256,
            ticks: 100,
            checkpoint_every: 50,
            commands: vec![LoggedCommand {
                tick: 7,
                command: SimCommand {
                    issuer: FactionId(1),
                    kind: CommandKind::Noop,
                },
            }],
            checkpoints: vec![
                Checkpoint { tick: 50, hash: 1 },
                Checkpoint {
                    tick: 100,
                    hash: u64::MAX,
                },
            ],
        }
    }

    #[test]
    fn ron_round_trip() {
        let replay = sample();
        assert_eq!(Replay::from_ron(&replay.to_ron()).unwrap(), replay);
    }

    #[test]
    fn rejects_other_formats() {
        let mut replay = sample();
        replay.format = REPLAY_FORMAT + 1;
        assert!(matches!(
            Replay::from_ron(&replay.to_ron()),
            Err(ReplayError::Format { .. })
        ));
    }

    #[test]
    fn finds_first_divergence() {
        let replay = sample();
        assert_eq!(replay.first_divergence(&replay.checkpoints), None);
        let actual = [
            Checkpoint { tick: 50, hash: 1 },
            Checkpoint { tick: 100, hash: 0 },
        ];
        assert_eq!(
            replay.first_divergence(&actual),
            Some((replay.checkpoints[1], Some(actual[1])))
        );
    }
}
