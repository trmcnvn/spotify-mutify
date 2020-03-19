use crate::spotify::Spotify;
use anyhow::Result;
use std::sync::mpsc::channel;
use std::thread;

#[cfg(target_os = "macos")]
mod macos;
mod spotify;
#[cfg(windows)]
mod windows;

fn main() -> Result<()> {
    let (tx, rx) = channel();
    let mut spotify = Spotify::new();

    // Watch Spotify and start/attach to the application
    let _watcher = Spotify::watch_data_directory(tx)?;
    spotify.run_or_attach()?;

    println!("Spotify ads will now be muted. Enjoy your music!");

    let mut is_muted = false;
    thread::spawn(move || {
        for event in rx {
            if let Ok(event) = event {
                if Spotify::is_valid_event(&event) {
                    let is_playing_ad = spotify.is_playing_ad();
                    if is_playing_ad && !is_muted {
                        is_muted = true;
                        spotify.set_mute(true).unwrap();
                    } else if !is_playing_ad && is_muted {
                        is_muted = false;
                        spotify.set_mute(false).unwrap();
                    }
                }
            }
        }
    })
    .join()
    .unwrap();

    Ok(())
}
