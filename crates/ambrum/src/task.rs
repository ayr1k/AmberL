// SPDX-License-Identifier: MPL-2.0
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Default)]
pub struct Cancellation {
    flag: Arc<AtomicBool>,
    parent: Option<Arc<Cancellation>>,
}
impl Cancellation {
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
    }
    pub fn check(&self) -> crate::Result<()> {
        if self.flag.load(Ordering::Acquire) {
            Err("操作已取消 / Cancelled".into())
        } else {
            self.parent.as_ref().map_or(Ok(()), |p| p.check())
        }
    }
    pub(crate) fn child(&self) -> Self {
        Self {
            flag: Arc::default(),
            parent: Some(Arc::new(self.clone())),
        }
    }
}
