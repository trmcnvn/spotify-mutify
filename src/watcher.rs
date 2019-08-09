use crossbeam_channel::Sender;
use directories::BaseDirs;
use failure::{format_err, Error};
use notify::{RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};

pub struct Watcher;
impl Watcher {
    pub fn watch(
        sender: Sender<notify::Result<notify::Event>>,
        username: String,
    ) -> Result<RecommendedWatcher, Error> {
        // Create an immediate watcher so events aren't debounced
        let mut watcher = notify::watcher(sender, std::time::Duration::from_millis(500))?;

        // Watch the target directory
        let base_dirs =
            BaseDirs::new().ok_or_else(|| format_err!("Couldn't find Base Directories"))?;
        let target_path = base_dirs
            .data_dir()
            .join(format!("Spotify\\Users\\{}-user", username));
        watcher.watch(&target_path, RecursiveMode::NonRecursive)?;
        Ok(watcher)
    }

    pub fn is_target_event(event: notify::Event) -> bool {
        event.paths.iter().any(|x| {
            if let Some(file_name) = x.file_name() {
                return file_name == "ad-state-storage.bnk" || file_name == "recently_played.bnk";
            }
            false
        })
    }
}
