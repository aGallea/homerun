use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunnerState {
    Creating,
    Registering,
    Online,
    Busy,
    Stopping,
    Offline,
    Error,
    Deleting,
}

impl RunnerState {
    pub fn can_transition_to(&self, next: &RunnerState) -> bool {
        use RunnerState::*;
        matches!(
            (self, next),
            (Creating, Registering)
                | (Registering, Online)
                | (Online, Busy)
                | (Busy, Online)
                | (Online, Offline)
                | (Online, Stopping)
                | (Busy, Stopping)
                | (Stopping, Offline)
                | (Stopping, Registering)
                | (Offline, Registering)
                | (Offline, Online)
                | (Offline, Deleting)
                | (Online, Deleting)
                | (Error, Deleting)
                | (Creating, Error)
                | (Registering, Error)
                | (Online, Error)
                | (Busy, Error)
                | (Stopping, Error)
                | (Error, Registering)
                | (Error, Offline)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        assert!(RunnerState::Creating.can_transition_to(&RunnerState::Registering));
        assert!(RunnerState::Registering.can_transition_to(&RunnerState::Online));
        assert!(RunnerState::Online.can_transition_to(&RunnerState::Busy));
        assert!(RunnerState::Busy.can_transition_to(&RunnerState::Online));
        assert!(RunnerState::Online.can_transition_to(&RunnerState::Offline));
        assert!(RunnerState::Busy.can_transition_to(&RunnerState::Stopping));
        assert!(RunnerState::Stopping.can_transition_to(&RunnerState::Offline));
    }

    #[test]
    fn test_invalid_transitions() {
        assert!(!RunnerState::Offline.can_transition_to(&RunnerState::Busy));
        assert!(!RunnerState::Creating.can_transition_to(&RunnerState::Busy));
    }

    #[test]
    fn test_any_state_can_error() {
        for state in [
            RunnerState::Creating,
            RunnerState::Registering,
            RunnerState::Online,
            RunnerState::Busy,
        ] {
            assert!(state.can_transition_to(&RunnerState::Error));
        }
    }
}
