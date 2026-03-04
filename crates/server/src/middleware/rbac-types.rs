/// Project roles from database enum
///
/// Permission hierarchy:
/// Owner > Admin > ProductOwner / Developer > BusinessAnalyst > QualityAssurance > Viewer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectRole {
    Owner,
    Admin,
    ProductOwner,
    Developer,
    BusinessAnalyst,
    QualityAssurance,
    Viewer,
}

impl ProjectRole {
    #[allow(dead_code)] // Reserved for serialization / API
    pub fn as_str(&self) -> &'static str {
        match self {
            ProjectRole::Owner => "owner",
            ProjectRole::Admin => "admin",
            ProjectRole::ProductOwner => "product_owner",
            ProjectRole::Developer => "developer",
            ProjectRole::BusinessAnalyst => "business_analyst",
            ProjectRole::QualityAssurance => "quality_assurance",
            ProjectRole::Viewer => "viewer",
        }
    }
}

/// Permission requirements for different operations
#[derive(Debug, Clone)]
pub enum Permission {
    /// Can view project and its resources
    ViewProject,
    /// Can modify project settings
    ManageProject,
    /// Can create tasks
    CreateTask,
    /// Can modify tasks (update, assign, change status)
    ModifyTask,
    /// Can delete tasks
    DeleteTask,
    /// Can execute tasks
    ExecuteTask,
    /// Can manage project members
    #[allow(dead_code)]
    ManageMembers,
    /// Can create requirements
    CreateRequirement,
    /// Can modify requirements
    ModifyRequirement,
    /// Can delete requirements
    DeleteRequirement,
    /// Can manage sprints
    ManageSprints,
    /// Can delete sprints (owner only)
    #[allow(dead_code)]
    DeleteSprint,
    /// Can view tasks (for viewing logs, approvals, etc.)
    ViewTask,
    /// Can approve tool executions (SDK mode)
    ApproveTools,
    /// Can view deployment environments, runs, releases (tab Deployments)
    ViewDeployments,
}

impl Permission {
    /// Get the minimum roles required for this permission (Permission Matrix)
    ///
    /// Role Capabilities:
    /// - Owner: All permissions
    /// - Admin: Administrative tasks (project, members, sprints)
    /// - ProductOwner: Same as BA – product decisions (sprints, requirements, tasks, approve)
    /// - BusinessAnalyst: Same as PO – requirements, tasks, sprints, approve
    /// - Developer: Implementation (execute tasks, modify code, approve tools)
    /// - QualityAssurance: Testing (create/edit/delete tasks, execute, approve attempts & tools)
    /// - Viewer: Read-only access
    pub fn required_roles(&self) -> &[ProjectRole] {
        match self {
            Permission::ViewProject => &[
                ProjectRole::Owner,
                ProjectRole::Admin,
                ProjectRole::ProductOwner,
                ProjectRole::Developer,
                ProjectRole::BusinessAnalyst,
                ProjectRole::QualityAssurance,
                ProjectRole::Viewer,
            ],
            Permission::ManageProject => &[ProjectRole::Owner, ProjectRole::Admin],
            Permission::CreateTask => &[
                ProjectRole::Owner,
                ProjectRole::Admin,
                ProjectRole::ProductOwner,
                ProjectRole::BusinessAnalyst,
                ProjectRole::Developer,
                ProjectRole::QualityAssurance,
            ],
            Permission::ModifyTask => &[
                ProjectRole::Owner,
                ProjectRole::Admin,
                ProjectRole::ProductOwner,
                ProjectRole::Developer,
                ProjectRole::BusinessAnalyst,
                ProjectRole::QualityAssurance,
            ],
            Permission::DeleteTask => &[
                ProjectRole::Owner,
                ProjectRole::Admin,
                ProjectRole::ProductOwner,
                ProjectRole::BusinessAnalyst,
                ProjectRole::Developer,
                ProjectRole::QualityAssurance,
            ],
            Permission::ExecuteTask => &[
                ProjectRole::Owner,
                ProjectRole::Admin,
                ProjectRole::ProductOwner,
                ProjectRole::Developer,
                ProjectRole::BusinessAnalyst,
                ProjectRole::QualityAssurance,
            ],
            Permission::ManageMembers => &[ProjectRole::Owner],
            Permission::CreateRequirement => &[
                ProjectRole::Owner,
                ProjectRole::Admin,
                ProjectRole::ProductOwner,
                ProjectRole::BusinessAnalyst,
            ],
            Permission::ModifyRequirement => &[
                ProjectRole::Owner,
                ProjectRole::Admin,
                ProjectRole::ProductOwner,
                ProjectRole::BusinessAnalyst,
            ],
            Permission::DeleteRequirement => &[ProjectRole::Owner, ProjectRole::Admin],
            Permission::ManageSprints => &[
                ProjectRole::Owner,
                ProjectRole::Admin,
                ProjectRole::ProductOwner,
                ProjectRole::BusinessAnalyst,
            ],
            Permission::DeleteSprint => &[ProjectRole::Owner],
            Permission::ViewTask => &[
                ProjectRole::Owner,
                ProjectRole::Admin,
                ProjectRole::ProductOwner,
                ProjectRole::Developer,
                ProjectRole::BusinessAnalyst,
                ProjectRole::QualityAssurance,
                ProjectRole::Viewer,
            ],
            Permission::ApproveTools => &[
                ProjectRole::Owner,
                ProjectRole::Admin,
                ProjectRole::ProductOwner,
                ProjectRole::BusinessAnalyst,
                ProjectRole::Developer,
                ProjectRole::QualityAssurance,
            ],
            Permission::ViewDeployments => &[
                ProjectRole::Owner,
                ProjectRole::Admin,
                ProjectRole::Developer,
            ],
        }
    }
}
