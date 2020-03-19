use crate::spotify::Spotify;
use anyhow::Result;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "macos")]
mod macos;
mod spotify;
#[cfg(windows)]
mod windows;

fn main() -> Result<()> {
    let (tx, rx) = flume::unbounded();
    let mut spotify = Spotify::new();

    // Watch Spotify and start/attach to the application
    let _watcher = Spotify::watch_data_directory(tx)?;
    spotify.run_or_attach()?;

    // Keep running while waiting for a termination signal
    let running = Arc::new(AtomicBool::new(true));
    let threaded_running = running.clone();
    ctrlc::set_handler(move || {
        threaded_running.store(false, Ordering::SeqCst);
    })?;

    println!("Spotify ads will now be muted. Enjoy your music!");

    let mut is_muted = false;
    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(100));
        if let Ok(event) = rx.try_recv() {
            let event = event?;
            if Spotify::is_valid_event(&event) {
                let is_playing_ad = spotify.is_playing_ad();
                if is_playing_ad && !is_muted {
                    is_muted = true;
                    spotify.set_mute(true)?;
                } else if !is_playing_ad && is_muted {
                    is_muted = false;
                    spotify.set_mute(false)?;
                }
            }
        }
    }

    Ok(())
}
