//! Interactive terminal progress bar for model download streaming.

use indicatif::{ProgressBar, ProgressStyle};

/// Creates a styled progress bar for downloading content of known or unknown length.
pub fn create_download_progress(total_bytes: Option<u64>, prefix: &str) -> ProgressBar {
    let pb = match total_bytes {
        Some(len) => {
            let pb = ProgressBar::new(len);
            let style = ProgressStyle::default_bar()
                .template("{prefix:.bold} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap_or_else(|_| ProgressStyle::default_bar())
                .progress_chars("#>-");
            pb.set_style(style);
            pb
        }
        None => {
            let pb = ProgressBar::new_spinner();
            let style = ProgressStyle::default_spinner()
                .template("{prefix:.bold} [{elapsed_precise}] {bytes} downloaded")
                .unwrap_or_else(|_| ProgressStyle::default_spinner());
            pb.set_style(style);
            pb
        }
    };
    pb.set_prefix(prefix.to_string());
    pb
}
