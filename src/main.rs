use crossbeam_channel::unbounded;
use notify::{RecursiveMode, RecommendedWatcher, Result, Watcher};

fn main() -> Result<()> {
    // Create a channel to receive events
    let (tx, rx) = unbounded();

    // Create a watcher to receive events immediatly rather than debounced
    let mut watcher: RecommendedWatcher = Watcher::new_immediate(tx)?;

    // Watch the target Spotify directory
    watcher.watch("%AppData%/Spotify/Users/vevix-user", RecursiveMode::NonRecursive)?;

    loop {
        match rx.recv() {
            Ok(event) => {},
            Err(err) => println!("watch error: {}", err),
        };
    }

    Ok(())
}
