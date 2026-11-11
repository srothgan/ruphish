//! Requests that belong to a page/row, rather than its rerunning effects or dialogs.
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation};

/// Capture once at the lifetime boundary of the state a request will update.
#[derive(Clone, Copy)]
pub struct ViewTasks(StoredValue<Owner>);
impl Default for ViewTasks {
    fn default() -> Self {
        Self(StoredValue::new(Owner::new()))
    }
}
impl ViewTasks {
    /// Keep work alive across effect/dialog cleanup, but cancel it on page removal.
    pub fn spawn(self, task: impl Future<Output = ()> + 'static) {
        self.0.with_value(|owner| {
            // A child owns each task's cleanup registration until that task ends.
            // Do not accumulate completed-task callbacks on the page owner.
            owner
                .child()
                .with(|| spawn_local_scoped_with_cancellation(task));
        });
    }
}
