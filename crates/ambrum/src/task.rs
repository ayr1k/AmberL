// SPDX-License-Identifier: MPL-2.0
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn check(&self) -> crate::Result<()> {
        if self.0.load(Ordering::Acquire) {
            Err("操作已取消 / Cancelled".into())
        } else {
            Ok(())
        }
    }
}
