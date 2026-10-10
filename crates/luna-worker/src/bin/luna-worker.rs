//! Stand-alone worker process (tests and tools). The app runs the same loop via `luna --worker`.

fn main() {
    luna_worker::worker_main();
}
