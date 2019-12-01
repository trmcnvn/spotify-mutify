#[cfg(windows)]
use crate::windows::*;
use anyhow::{anyhow, Result};
use directories::BaseDirs;
#[cfg(windows)]
use external::module::*;
#[cfg(windows)]
use external::process::*;
#[cfg(windows)]
use notify::Watcher;
#[cfg(windows)]
use pelite::{pattern, PeFile};
use std::path::PathBuf;
use std::time::Duration;

type SenderType = crossbeam_channel::Sender<notify::Result<notify::Event>>;

pub(crate) struct Spotify {
    #[cfg(windows)]
    process: Option<Process>,
    #[cfg(windows)]
    target_address: usize,
    #[cfg(windows)]
    windows_com: Windows,
}

impl Spotify {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            process: None,
            #[cfg(windows)]
            target_address: 0,
            #[cfg(windows)]
            windows_com: Windows::new(),
        }
    }

    /// Watch the Spotify data directory for changes
    pub fn watch_data_directory(&self, sender: SenderType) -> Result<notify::RecommendedWatcher> {
        let mut watcher = notify::watcher(sender, Duration::from_millis(500))?;

        // Watch each `-user` directory within the data directory This is easier than having the user specify which user
        // is currently listening
        let target_path = self.find_data_directory()?;
        for entry in std::fs::read_dir(&target_path)? {
            let entry = entry?;
            let path = entry
                .path()
                .into_os_string()
                .into_string()
                .map_err(|err| anyhow!("{:?}", err))?;
            if path.contains("-user") {
                watcher.watch(path, notify::RecursiveMode::NonRecursive)?;
            }
        }

        Ok(watcher)
    }

    pub fn is_valid_event(&self, event: &notify::Event) -> bool {
        event.paths.iter().any(|x| {
            if let Some(file_name) = x.file_name() {
                return file_name == "ad-state-storage.bnk" || file_name == "recently_played.bnk";
            }
            false
        })
    }

    pub fn is_playing_ad(&self) -> bool {
        if let Ok(track) = self.get_current_track() {
            return track.eq("spotify:ad");
        }
        false
    }

    #[cfg(windows)]
    pub fn run_or_attach(&mut self) -> Result<()> {
        // Find `spotify.exe` within the currently running processes
        let find_process_fn = || -> Result<ProcessEntry> {
            let mut processes = EnumProcess::create()?;
            processes
                .find(|process| {
                    if let Ok(name) = process.exe_file().into_string() {
                        if name.to_lowercase().contains("spotify.exe") {
                            return true;
                        }
                    }
                    false
                })
                .ok_or_else(|| anyhow!("Couldn't find Spotify process"))
        };
        #[allow(clippy::single_match_else)]
        let process = match find_process_fn() {
            Ok(process) => process,
            _ => {
                std::process::Command::new("spotify").spawn()?;
                std::thread::sleep(Duration::from_secs(2)); // Wait for spawn...
                find_process_fn()?
            }
        };

        // Find the target module within the Spotify process
        let mut modules = EnumModules::create(process.process_id())?;
        let module = modules
            .find(|module| {
                if let Ok(name) = module.name().into_string() {
                    if name.to_lowercase().contains("chrome_elf.dll") {
                        return true;
                    }
                }
                false
            })
            .ok_or_else(|| anyhow!("Couldn't find target module within the Spotify process"))?;

        // Attach to process
        let rights = ProcessRights::new().vm_read().query_limited_information();
        let process = Process::attach(process.process_id(), rights)?;
        self.process = Some(process.clone());

        // Load the module into a PeFile structure
        let mut bytes: Vec<u8> = vec![0; module.size()];
        process.vm_read_partial(module.base(), &mut bytes)?;
        let file = PeFile::from_bytes(&bytes)?;

        // Search for the memory pattern
        let pattern = pattern!("01 00 00 00 '73 70 6F 74 69 66 79 3A");
        let mut addresses = [0; 2];
        file.scanner()
            .matches(pattern, file.headers().image_range())
            .next(&mut addresses);
        self.target_address = (module.base() + addresses[1] as usize) - 0x1400; // TODO: ???

        // Continue looking for the volume control until it is found. It won't exist until Spotify
        // actually starts playing.
        loop {
            let pid = process.pid()?;
            match self.windows_com.find_audio_control(pid) {
                Ok(_) => break,
                _ => std::thread::sleep(Duration::from_secs(1)),
            };
        }

        Ok(())
    }

    #[cfg(windows)]
    pub fn set_mute(&self, value: bool) -> Result<()> {
        self.windows_com.set_mute(value)
    }

    #[cfg(windows)]
    fn get_current_track(&self) -> Result<String> {
        let mut data = [0; 10];
        if let Some(process) = &self.process {
            process.vm_read_partial(self.target_address, &mut data)?;
        }
        let current_track = std::str::from_utf8(&data)?;
        Ok(current_track.to_owned())
    }

    /// Find the directory for Spotify's local data
    fn find_data_directory(&self) -> Result<PathBuf> {
        let base_directory =
            BaseDirs::new().ok_or_else(|| anyhow!("Couldn't find valid home directory"))?;

        // Check the default data directory path
        // Linux    $XDG_DATA_HOME or $HOME/.local/share
        // macOS	$HOME/Library/Application Support
        // Windows	{FOLDERID_RoamingAppData}
        let target_path = base_directory.data_dir().join("Spotify\\Users");
        if target_path.as_path().exists() {
            return Ok(target_path);
        }

        // Windows Store installs use a unique path
        if cfg!(windows) {
            // {FOLDERID_LocalAppData}
            let target_path = base_directory.data_local_dir().join("Packages");
            if target_path.as_path().exists() {
                for entry in std::fs::read_dir(&target_path)? {
                    let entry = entry?;
                    let path = entry
                        .path()
                        .into_os_string()
                        .into_string()
                        .map_err(|err| anyhow!("{:?}", err))?;
                    if path.contains("SpotifyAB.SpotifyMusic") {
                        let target_path = entry.path().join("LocalState\\Spotify\\Users");
                        if target_path.as_path().exists() {
                            return Ok(target_path);
                        }
                    }
                }
            }
        }

        Err(anyhow!("Couldn't find the Spotify data directory"))
    }
}
