// SPDX-License-Identifier: MIT
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(debug_assertions, derive(Debug))]
#[serde(into = "u8", try_from = "u8")]
#[repr(u8)]
pub enum State {
    /// System up to date, nothing to do.
    Normal,
    /// New update installed, commit to continue.
    Installed,
    /// Update committed, reboot to test it.
    Committed,
    /// Update in progress, call update finish.
    Testing,
    /// Currently moving back to an older system, please reboot.
    Revert,
}

impl Default for State {
    fn default() -> Self {
        State::Normal
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Normal => write!(f, "System up to date, nothing to do."),
            Self::Installed => write!(f, "New update installed, commit to continue."),
            Self::Committed => write!(f, "Update committed, reboot to test it."),
            Self::Testing => write!(f, "Update in progress, call update finish."),
            Self::Revert => write!(
                f,
                "Currently moving back to an older system, please reboot."
            ),
        }
    }
}

/// Allow deserialization of the state from a byte.
impl From<State> for u8 {
    fn from(value: State) -> u8 {
        value as u8
    }
}

/// Attempt deserialization of the state from a byte.
impl TryFrom<u8> for State {
    type Error = serde::de::value::Error;

    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::Normal),
            1 => Ok(Self::Installed),
            2 => Ok(Self::Committed),
            3 => Ok(Self::Testing),
            4 => Ok(Self::Revert),
            _ => Err(<Self::Error as serde::de::Error>::custom("invalid state")),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use bincode::Options;

    /// Test default initialization of a state.
    #[test]
    fn test_state_default() {
        assert_eq!(State::default(), State::Normal);
    }

    /// Test serialization of a state.
    #[test]
    fn test_state_serialize() {
        let default_state = State::default();
        let serialized = bincode::options()
            .with_fixint_encoding()
            .serialize(&default_state)
            .unwrap();

        let expected = [0u8; 1];

        assert_eq!(serialized.as_slice(), &expected);
    }

    /// Test conversion of a state from byte.
    #[test]
    fn test_from_u8() {
        let state = State::try_from(2u8).unwrap();

        assert_eq!(state, State::Committed);
    }
}
