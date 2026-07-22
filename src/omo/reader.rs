use crate::omo::types::*;

pub trait PlanReader: Send + Sync {
    fn list_plans(&self) -> Result<Vec<OmoPlan>, OmoError>;
    fn read_plan(&self, slug: &str) -> Result<OmoPlan, OmoError>;
}
