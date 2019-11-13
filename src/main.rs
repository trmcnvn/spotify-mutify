#![deny(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use crate::watcher::*;
#[cfg(windows)]
use crate::windows::*;
use crossbeam_channel::{select, unbounded, Receiver};
#[cfg(windows)]
use ctrlc::set_handler;
use failure::Error;
#[cfg(windows)]
use pelite::{pattern, PeFile};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

mod watcher;
#[cfg(windows)]
mod windows;

fn welcome_message() {
    println!("Spotify ads will now be muted. Enjoy your music!");
}

fn main() -> Result<(), Error> {
    // Watch the Spotify directory for file changes
    let (tx, rx) = unbounded();
    let _watcher = Watcher::watch(tx)?;

    // Continuously run and watch for interrupts to exit cleanly
    let running = Arc::new(AtomicBool::new(true));
    let running_thd = running.clone();
    set_handler(move || {
        running_thd.store(false, Ordering::SeqCst);
    })?;

    // Enjoy your music
    mute_spotify(running, rx)
}

#[cfg(windows)]
fn mute_spotify(
    running: Arc<AtomicBool>,
    rx: Receiver<notify::Result<notify::Event>>,
) -> Result<(), Error> {
    // Find Spotify and attach to the process
    let (process_entry, module_entry) = Windows::find_spotify()?;
    let process = Windows::attach_to_spotify(process_entry.process_id())?;

    // Read the target modules data into memory
    let mut bytes: Vec<u8> = vec![0; module_entry.size()];
    process.vm_read_partial(module_entry.base(), &mut bytes)?;

    // Map the read data into a PeFile structure and scan for our signature address
    let file = PeFile::from_bytes(&bytes)?;
    let pattern = pattern!("01 00 00 00 '73 70 6F 74 69 66 79 3A");
    let mut addresses = [0; 2];
    file.scanner()
        .matches(&pattern, file.headers().image_range())
        .next(&mut addresses);
    let target_address = (module_entry.base() + addresses[1] as usize) - 0x1400; // This needs to be fixed!

    println!(
        "Mem. Read: {:?}",
        Windows::get_current_track(&process, target_address)?
    );

    // Get audio session control
    let com = Windows::get_audio_session(process_entry.process_id())?;
    welcome_message();

    // Block for events from the watcher
    let mut is_muted = false;
    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(100));
        select! {
            recv(rx) -> event => {
                if let Ok(event) = event {
                    if let Ok(event) = event {
                        if Watcher::is_target_event(event) {
                            let identifier = Windows::get_current_track(&process, target_address)?;
                            let is_playing_ad = identifier.eq("spotify:ad");
                            if is_playing_ad && !is_muted {
                                is_muted = true;
                                com.set_mute(is_playing_ad as i32)?;
                            } else if !is_playing_ad && is_muted {
                                is_muted = false;
                                com.set_mute(is_playing_ad as i32)?;
                            }
                        }
                    }
                }
            },
            default => {},
        };
    }
    Ok(())
}
