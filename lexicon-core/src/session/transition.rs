use crate::session::error::SessionTransitionError;
use crate::session::model::{SessionState, SessionTransition};

/// Validate that transitioning from `current` state with `transition` is legal.
///
/// Allowed transitions:
/// - Prepared  → Running
/// - Prepared  → Failed
/// - Prepared  → Abandoned
/// - Running   → Succeeded
/// - Running   → Failed
/// - Running   → Abandoned
/// - Failed    → Abandoned
///
/// All other transitions (including back to Prepared, repeated terminal transitions,
/// Failed → Running, Succeeded → *, Abandoned → *) are rejected.
pub(crate) fn validate_transition(
    current: SessionState,
    transition: &SessionTransition,
) -> Result<(), SessionTransitionError> {
    let target = transition.target_state();

    if current.is_terminal() && target != SessionState::Abandoned {
        if current == SessionState::Succeeded || current == SessionState::Abandoned {
            return Err(SessionTransitionError::TerminalStateReached { state: current });
        }
    }

    let allowed = match (current, target) {
        (SessionState::Prepared, SessionState::Running) => true,
        (SessionState::Prepared, SessionState::Failed) => true,
        (SessionState::Prepared, SessionState::Abandoned) => true,
        (SessionState::Running, SessionState::Succeeded) => true,
        (SessionState::Running, SessionState::Failed) => true,
        (SessionState::Running, SessionState::Abandoned) => true,
        (SessionState::Failed, SessionState::Abandoned) => true,
        _ => false,
    };

    if !allowed {
        return Err(SessionTransitionError::InvalidTransition {
            from: current,
            to: target,
        });
    }

    Ok(())
}
