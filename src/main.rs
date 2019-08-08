use crate::watcher::*;
#[cfg(windows)]
use crate::windows::*;
use crossbeam_channel::{unbounded, Receiver};
use failure::Error;
use notify::RawEvent;
#[cfg(windows)]
use pelite::{pattern, PeFile};
use structopt::StructOpt;

mod watcher;
#[cfg(windows)]
mod windows;

#[derive(StructOpt, Debug)]
#[structopt(name = "mutify")]
struct CommandOptions {
    #[structopt(short, long, help = "Spotify username")]
    username: String,
}

fn main() -> Result<(), Error> {
    let args = CommandOptions::from_args();

    // Create communication channel
    let (tx, rx) = unbounded();

    // Watch the Spotify directory for file changes
    let _watcher = Watcher::watch(tx, args.username)?;

    // OS-specific
    dont_play_your_ads_at_a_higher_volume(rx)
}

#[cfg(windows)]
fn dont_play_your_ads_at_a_higher_volume(rx: Receiver<RawEvent>) -> Result<(), Error> {
    // Find Spotify and attach to the process
    let (process_entry, module_entry) = Windows::find_spotify()?;
    let process = Windows::attach_to_spotify(process_entry.process_id())?;

    // Read the target modules data into memory
    let mut bytes: Vec<u8> = vec![0; module_entry.size()];
    process.vm_read_partial(module_entry.base(), &mut bytes)?;

    // Map the read data into a PeFile structure and scan for our signature address
    // TODO: Figure out why this finds the signature 0x1800 past the actual point.
    let file = PeFile::from_bytes(&bytes)?;
    let pattern = pattern!("01 00 00 00 '73 70 6F 74 69 66 79 3A");
    let mut addresses = [0; 2];
    file.scanner()
        .matches(&pattern, file.headers().image_range())
        .next(&mut addresses);

    // Wait for events from the watcher
    let mut is_playing_ad = false;
    loop {
        if let Ok(event) = rx.recv() {
            if Watcher::is_target_event(event) {
                let identifier = Windows::get_current_track(
                    &process,
                    (module_entry.base() + addresses[1] as usize) - 0x1800,
                )?;
                let result = identifier.contains("spotify:ad");
                if result && !is_playing_ad {
                    is_playing_ad = true;
                    Windows::mute_spotify(process_entry.process_id(), result)?;
                } else if !result && is_playing_ad {
                    is_playing_ad = false;
                    Windows::mute_spotify(process_entry.process_id(), result)?;
                }
            }
        }
    }
}
