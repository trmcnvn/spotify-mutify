#![deny(clippy::all, clippy::pedantic, clippy::nursery)]

use crate::spotify::Spotify;
use anyhow::Result;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

mod spotify;
#[cfg(windows)]
mod windows;

fn main() -> Result<()> {
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut spotify = Spotify::new();

    // Watch Spotify and start/attach to the application
    let _watcher = spotify.watch_data_directory(tx)?;
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
        crossbeam_channel::select! {
            recv(rx) -> event => {
                let event = event??;
                if spotify.is_valid_event(&event) {
                    let is_playing_ad = spotify.is_playing_ad();
                    if is_playing_ad && !is_muted {
                        is_muted = true;
                        spotify.set_mute(true)?;
                    } else if !is_playing_ad && is_muted {
                        thread::sleep(Duration::from_millis(500));
                        is_muted = false;
                        spotify.set_mute(false)?;
                    }
                }
            },
            default => {},
        };
    }

    Ok(())
}
