use std::sync::OnceLock;

use tokio::runtime::{Builder, Runtime};

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        Builder::new_multi_thread()
            .worker_threads(crate::worker_threads())
            .max_blocking_threads(crate::blocking_threads())
            .thread_name("quark-worker")
            .enable_all()
            .build()
            .expect("failed to start the Quark runtime")
    })
}
