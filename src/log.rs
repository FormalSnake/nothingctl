use std::sync::atomic::{AtomicBool, Ordering};

static VERBOSE: AtomicBool = AtomicBool::new(false);

pub fn set_verbose(on: bool) {
    VERBOSE.store(on, Ordering::Relaxed);
}

pub fn verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

/// Debug line on stderr, shown with `--verbose`.
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        if $crate::log::verbose() {
            eprintln!("debug: {}", format_args!($($arg)*));
        }
    };
}
