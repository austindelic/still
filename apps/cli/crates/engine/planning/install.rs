//! Side-effect-free plans produced before execution.

use crate::actions::install::{InstallRequest, classify_install_items};
use crate::error::{EngineError, EngineResult};
use crate::specs::item::{ItemKind, ItemSpec};

/// Plan for installing requested tools, packages, and apps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlan {
    operations: Vec<PlanOperation>,
}

impl InstallPlan {
    /// Builds an install plan from a parsed install request.
    /// # Errors
    /// Fails when the request contains no items.
    pub fn from_request(request: InstallRequest) -> EngineResult<Self> {
        if request.items.is_empty() {
            return Err(EngineError::EmptyInstallRequest);
        }

        let items = classify_install_items(&request.items)?;

        Ok(Self {
            operations: items
                .into_iter()
                .map(|item| PlanOperation::Install {
                    kind: item.kind,
                    spec: item.spec,
                })
                .collect(),
        })
    }

    /// Returns the operations that need executor side effects.
    pub fn operations(&self) -> &[PlanOperation] {
        &self.operations
    }
}

/// Planned operation that an executor can apply later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanOperation {
    Install { kind: ItemKind, spec: ItemSpec },
}

#[cfg(test)]
mod tests {
    use crate::actions::install::InstallItemRequest;

    use super::*;

    #[test]
    fn install_plan_preserves_requested_order_and_kind() {
        let plan = InstallPlan::from_request(InstallRequest {
            items: vec![
                item(ItemKind::Tool, "jq"),
                item(ItemKind::Package, "openssl@3.0.0@homebrew"),
                item(ItemKind::App, "firefox@latest@homebrew-cask"),
            ],
        })
        .unwrap();

        assert_eq!(plan.operations().len(), 3);
        assert!(matches!(
            &plan.operations()[0],
            PlanOperation::Install {
                kind: ItemKind::Tool,
                spec
            } if spec.name == "jq"
        ));
        assert!(matches!(
            &plan.operations()[1],
            PlanOperation::Install {
                kind: ItemKind::Package,
                spec
            } if spec.backend.as_ref().unwrap().as_str() == "homebrew"
        ));
        assert!(matches!(
            &plan.operations()[2],
            PlanOperation::Install {
                kind: ItemKind::App,
                spec
            } if spec.backend.as_ref().unwrap().as_str() == "homebrew-cask"
        ));
    }

    #[test]
    fn install_plan_rejects_empty_requests() {
        let err = InstallPlan::from_request(InstallRequest { items: Vec::new() }).unwrap_err();

        assert!(matches!(
            err,
            crate::error::EngineError::EmptyInstallRequest
        ));
    }

    fn item(kind: ItemKind, spec: &str) -> InstallItemRequest {
        InstallItemRequest {
            kind: Some(kind),
            spec: spec.parse().unwrap(),
            tool: Default::default(),
        }
    }
}
