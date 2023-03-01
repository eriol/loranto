use std::thread;
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};
use tokio::runtime;
use tokio::time::interval;

// Show a progress bar that will be full in `scan_time` seconds.
pub fn progress_bar(scan_time: Duration) {
    let steps = (scan_time.as_millis() / 5) as u64;
    let pb = ProgressBar::new(steps);
    let spinner_style = ProgressStyle::with_template("{spinner} [{wide_bar}]")
        .unwrap()
        .progress_chars("#>-");
    pb.set_style(spinner_style);

    let rt = runtime::Builder::new_multi_thread()
        .enable_time()
        .build()
        .expect("failed to create runtime");

    let future = async move {
        pb.set_message("Scanning...");
        let mut intv = interval(Duration::from_millis(5));

        for _ in 0..steps {
            intv.tick().await;
            pb.inc(1);
        }
        pb.finish_with_message("Done");
    };
    thread::spawn(move || {
        rt.block_on(future);
    });
}
