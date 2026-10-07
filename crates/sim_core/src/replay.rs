//! Replay files: seed + recorded command stream + expected state hashes
//! (NFR-08, ADR 0004). Stored as RON so a divergence can be read and edited.

use serde::{Deserialize, Serialize};

use crate::command::LoggedCommand;
use crate::run::RunConfig;

/// Bump when the replay format or the meaning of a command changes.
/// 5 (M4): the `run` header field, and the run gate and new commands.
pub const REPLAY_FORMAT: u32 = 5;

/// The state hash at a given tick (after `tick` ticks have run).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub tick: u64,
    pub hash: u64,
}

/// A recorded session: setup, the player's commands and the expected hashes (NFR-08).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Replay {
    pub format: u32,
    pub seed: u64,
    /// Villagers populated (villagers.ron's total by default).
    pub population: u32,
    /// Map size in tiles (worldgen settings otherwise come from the build).
    pub world_size: u32,
    /// Scenario set up before the first tick ("" = villages only;
    /// "skirmish" = the M3 playtest armies).
    #[serde(default)]
    pub scenario: String,
    /// The run's configuration, for a run (GP-21); `None` otherwise.
    #[serde(default)]
    pub run: Option<RunConfig>,
    pub ticks: u64,
    /// Ticks between checkpoints (0 = only the final state).
    pub checkpoint_every: u64,
    pub commands: Vec<LoggedCommand>,
    /// Expected hashes, in tick order. The last one is the final state.
    pub checkpoints: Vec<Checkpoint>,
}

/// Why a replay file could not be loaded.
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
    /// Serialises to pretty RON.
    pub fn to_ron(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .expect("replay types always serialize")
    }

    /// Parses RON and checks the format version.
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
    use crate::{CommandKind, Difficulty, FactionId, SimCommand};

    fn sample() -> Replay {
        Replay {
            format: REPLAY_FORMAT,
            seed: 42,
            population: 8,
            world_size: 256,
            scenario: "run".into(),
            run: Some(RunConfig {
                difficulty: Difficulty::Harsh,
            }),
            ticks: 100,
            checkpoint_every: 50,
            commands: vec![
                LoggedCommand {
                    tick: 0,
                    command: SimCommand::player(
                        FactionId::PLAYER,
                        CommandKind::ChooseOffer {
                            offer: 0,
                            choice: 2,
                        },
                    ),
                },
                LoggedCommand {
                    tick: 7,
                    command: SimCommand::player(FactionId::PLAYER, CommandKind::Noop),
                },
            ],
            checkpoints: vec![
                Checkpoint { tick: 50, hash: 1 },
                Checkpoint {
                    tick: 100,
                    hash: u64::MAX,
                },
            ],
        }
    }

    /// GP-21: a run's config survives the round trip, and replays without
    /// one read back as `None`.
    #[test]
    fn ron_round_trip() {
        let replay = sample();
        assert_eq!(Replay::from_ron(&replay.to_ron()).unwrap(), replay);

        let plain = Replay {
            scenario: String::new(),
            run: None,
            ..sample()
        };
        let text = plain.to_ron();
        assert_eq!(Replay::from_ron(&text).unwrap(), plain);
        let without_field = text.replace("run: None,", "");
        assert_ne!(without_field, text);
        assert_eq!(Replay::from_ron(&without_field).unwrap().run, None);
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
