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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    UseApp,
    ReadSource,
    Fork,
    RunJob,
    CancelRun,
    PushBranch,
    OpenPullRequest,
    ReviewPullRequest,
    Merge,
    Publish,
    ManageSchedules,
    ReadSecretMetadata,
    WriteSecrets,
    ManageProject,
    ManageMembers,
    ViewAuditLog,
    DeleteProject,
}

impl Role {
    pub fn allows(self, action: Action) -> bool {
        let minimum = match action {
            Action::UseApp => Role::Consumer,
            Action::ReadSource | Action::Fork => Role::Reader,
            Action::RunJob | Action::CancelRun => Role::Runner,
            Action::PushBranch | Action::OpenPullRequest | Action::ReviewPullRequest => {
                Role::Contributor
            }
            Action::Merge
            | Action::Publish
            | Action::ManageSchedules
            | Action::ReadSecretMetadata
            | Action::WriteSecrets
            | Action::ManageProject => Role::Maintainer,
            Action::ManageMembers | Action::ViewAuditLog | Action::DeleteProject => Role::Owner,
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

    #[test]
    fn every_role_has_a_strict_boundary() {
        let cases = [
            (Role::Consumer, Action::UseApp, Action::ReadSource),
            (Role::Reader, Action::ReadSource, Action::RunJob),
            (Role::Runner, Action::CancelRun, Action::PushBranch),
            (
                Role::Contributor,
                Action::ReviewPullRequest,
                Action::Publish,
            ),
            (
                Role::Maintainer,
                Action::ManageProject,
                Action::ManageMembers,
            ),
        ];
        for (role, allowed, denied) in cases {
            assert!(role.allows(allowed), "{role:?} should allow {allowed:?}");
            assert!(!role.allows(denied), "{role:?} should deny {denied:?}");
        }
        assert!(Role::Owner.allows(Action::DeleteProject));
    }

    #[test]
    fn action_wire_names_are_stable() {
        #[derive(Serialize)]
        struct Wire {
            action: Action,
        }
        assert_eq!(
            toml::to_string(&Wire {
                action: Action::OpenPullRequest
            })
            .unwrap()
            .trim(),
            "action = \"open_pull_request\""
        );
    }
}
