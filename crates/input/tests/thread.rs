//! The input thread starts SDL 3.4 on a thread of its own and stops it
//! cleanly, on every OS CI runs (ADR-0018). No device is needed: GitHub's
//! machines have none. Basis: Rule.

use std::time::{Duration, Instant};

use opendrone_input::InputThread;

#[test]
fn sdl_starts_on_its_own_thread_polls_and_stops_cleanly() {
    let thread = InputThread::start().expect("SDL starts");
    assert!(
        thread.sdl_version().starts_with("3.4."),
        "{}",
        thread.sdl_version()
    );
    // Only one runs at a time: SDL is one per program.
    assert!(InputThread::start().is_err());
    let waited = Instant::now();
    while thread.polls() < 10 && waited.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(thread.polls() >= 10, "the thread keeps reading SDL");
    // Whatever devices this computer has, every batch carries a stamp from
    // the thread's clock, no later than now.
    let now = thread.now();
    for batch in thread.batches() {
        assert!(batch.at <= now);
    }
    drop(thread);
    // Once stopped, SDL can start again.
    drop(InputThread::start().expect("SDL starts again"));
}
