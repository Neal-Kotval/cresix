use serde::{Deserialize, Serialize};

/// Project roles are intentionally cumulative. A higher role includes every
/// capability of the roles below it, making invitations easy to understand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Consumer,
    Reader,
    Runner,
    Contributor,
    Maintainer,
    Owner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    UseApp,
    ReadSource,
    Fork,
    RunJob,
    PushBranch,
    OpenPullRequest,
    Merge,
    Publish,
    ManageSchedules,
    WriteSecrets,
    ManageMembers,
    DeleteProject,
}

impl Role {
    pub fn allows(self, action: Action) -> bool {
        let minimum = match action {
            Action::UseApp => Role::Consumer,
            Action::ReadSource | Action::Fork => Role::Reader,
            Action::RunJob => Role::Runner,
            Action::PushBranch | Action::OpenPullRequest => Role::Contributor,
            Action::Merge | Action::Publish | Action::ManageSchedules | Action::WriteSecrets => {
                Role::Maintainer
            }
            Action::ManageMembers | Action::DeleteProject => Role::Owner,
        };
        self >= minimum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_are_cumulative() {
        assert!(Role::Owner.allows(Action::UseApp));
        assert!(Role::Maintainer.allows(Action::Publish));
        assert!(!Role::Contributor.allows(Action::Publish));
        assert!(!Role::Consumer.allows(Action::ReadSource));
    }
}
