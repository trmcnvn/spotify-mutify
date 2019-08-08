use external::module::*;
use external::process::*;
use failure::{format_err, Error};

pub struct Windows;

impl Windows {
    /// Get a handle to the process with access to read memory
    pub fn attach_to_spotify(pid: ProcessId) -> Result<Process, Error> {
        let rights = ProcessRights::new().vm_read();
        let process = Process::attach(pid, rights)?;
        Ok(process)
    }

    /// Returns a tuple of the Spotify process and `chrome_elf` module.
    pub fn find_spotify() -> Result<(ProcessEntry, ModuleEntry), Error> {
        let process = Windows::find_process()?;
        let module = Windows::find_module(process.process_id())?;
        Ok((process, module))
    }

    /// Gets the internal URI for the currently playing track
    pub fn get_current_track(process: &Process, address: usize) -> Result<String, Error> {
        let mut uri = [0; 10];
        process.vm_read_partial(address, &mut uri)?;

        let currently_playing = std::str::from_utf8(&uri)?;
        Ok(currently_playing.to_owned())
    }

    /// Mute the Spotify application
    pub fn mute_spotify(pid: ProcessId, playing_ad: bool) -> Result<(), Error> {
        Ok(())
    }

    /// Finds the `chrome_elf.dll` module within the Spotify process. This contains the memory
    /// we are looking to read.
    fn find_module(pid: ProcessId) -> Result<ModuleEntry, Error> {
        let mut modules = EnumModules::create(pid)?;
        modules
            .find(|module| {
                if let Ok(name) = module.name().into_string() {
                    if name.to_lowercase().contains("chrome_elf.dll") {
                        return true;
                    }
                }
                false
            })
            .ok_or_else(|| format_err!("Couldn't find `chrome_elf.dll` within Spotify"))
    }

    /// Finds the `Spotify.exe` process and returns an ProcessEntry instance
    fn find_process() -> Result<ProcessEntry, Error> {
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
            .ok_or_else(|| format_err!("Couldn't find the Spotify process"))
    }
}
