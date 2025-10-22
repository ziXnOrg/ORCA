use casbin::{CoreApi, Enforcer};
use std::sync::Arc;

#[derive(Clone)]
pub struct Rbac {
    enforcer: Arc<Enforcer>,
}

impl Rbac {
    pub async fn from_files(model_path: &str, policy_path: &str) -> anyhow::Result<Self> {
        let enforcer = Enforcer::new(model_path, policy_path).await?;
        Ok(Self { enforcer: Arc::new(enforcer) })
    }

    pub async fn allow(&self, sub: &str, obj: &str, act: &str) -> bool {
        self.enforcer.enforce((sub, obj, act)).unwrap_or(false)
    }
}
