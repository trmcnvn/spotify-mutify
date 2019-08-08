use crossbeam_channel::Sender;
use directories::BaseDirs;
use failure::{format_err, Error};
use notify::{RawEvent, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};

pub struct Watcher;

impl Watcher {
    pub fn watch(
        sender: Sender<notify::RawEvent>,
        username: String,
    ) -> Result<RecommendedWatcher, Error> {
        // Create an immediate watcher so events aren't debounced
        let mut watcher: RecommendedWatcher = NotifyWatcher::new_immediate(sender)?;

        // Watch the target directory
        let base_dirs =
            BaseDirs::new().ok_or_else(|| format_err!("Couldn't find Base Directories"))?;
        let target_path = base_dirs
            .config_dir()
            .join(format!("Spotify\\Users\\{}-user", username));
        watcher.watch(&target_path, RecursiveMode::NonRecursive)?;
        Ok(watcher)
    }

    pub fn is_target_event(event: RawEvent) -> bool {
        if let Some(file_name) = event.path.unwrap().file_name() {
            return match file_name.to_str() {
                Some("ad-state-storage.bnk.tmp")
                | Some("recently_played.bnk.tmp")
                | Some("ad-state-storage.bnk")
                | Some("recently_played.bnk") => true,
                _ => false,
            };
        }
        false
    }
}
