use com_ptr::*;
use external::module::*;
use external::process::*;
use external::FromInner;
use failure::{format_err, Error};
use std::ptr;
use winapi::{
    shared::{minwindef::DWORD, winerror::S_OK},
    um::{
        audioclient::{ISimpleAudioVolume, AUDCLNT_S_NO_SINGLE_PROCESS},
        audiopolicy::{
            IAudioSessionControl, IAudioSessionControl2, IAudioSessionEnumerator,
            IAudioSessionManager2, IID_IAudioSessionManager2,
        },
        combaseapi::{CoInitializeEx, CoUninitialize, CLSCTX_ALL},
        mmdeviceapi::{
            eMultimedia, eRender, CLSID_MMDeviceEnumerator, IMMDevice, IMMDeviceEnumerator,
        },
        objbase::COINIT_APARTMENTTHREADED,
    },
};

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

    /// ...
    pub fn get_audio_session(pid: ProcessId) -> Result<ComIsBleh, Error> {
        let mut com = ComIsBleh::new();
        let device = com.get_device()?;
        let session_enumerator = com.get_session_enumerator(&device)?;
        let session_count = com.get_session_count(&session_enumerator)?;

        // Find the session for Spotify
        for idx in 0..session_count {
            let control = com.get_session_control(&session_enumerator, idx)?;
            let process_id = com.get_session_pid(&control)?;

            // This is the session for Spotify
            if unsafe { ProcessId::from_inner(process_id) } == pid {
                com.set_session_control(&control)?;
                break;
            }
        }
        Ok(com)
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

// Handle uninitialization of COM 🤢
pub struct ComIsBleh {
    control: Option<ComPtr<ISimpleAudioVolume>>,
}

impl ComIsBleh {
    fn new() -> Self {
        unsafe { CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED) };
        Self { control: None }
    }

    pub fn get_device(&self) -> Result<ComPtr<IMMDevice>, Error> {
        let device_enumerator =
            co_create_instance::<IMMDeviceEnumerator>(&CLSID_MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|err| {
                    format_err!("co_create_instance::<IMMDeviceEnumerator>: {:08x}", err)
                })?;

        ComPtr::new(|| {
            let mut obj = ptr::null_mut();
            let res = unsafe {
                device_enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia, &mut obj)
            };
            hresult(obj as *mut IMMDevice, res)
        })
        .map_err(|err| format_err!("IMMDeviceEnumerator::GetDefaultAudioEndpoint: {:08x}", err))
    }

    pub fn get_session_enumerator(
        &self,
        device: &ComPtr<IMMDevice>,
    ) -> Result<ComPtr<IAudioSessionEnumerator>, Error> {
        let session_manager = ComPtr::new(|| {
            let mut obj = ptr::null_mut();
            let res = unsafe {
                device.Activate(
                    &IID_IAudioSessionManager2,
                    CLSCTX_ALL,
                    ptr::null_mut(),
                    &mut obj,
                )
            };
            hresult(obj as *mut IAudioSessionManager2, res)
        })
        .map_err(|err| format_err!("IMMDevice::Activate: {:08x}", err))?;

        // Get the active sessions
        ComPtr::new(|| {
            let mut obj = ptr::null_mut();
            let res = unsafe { session_manager.GetSessionEnumerator(&mut obj) };
            hresult(obj as *mut IAudioSessionEnumerator, res)
        })
        .map_err(|err| format_err!("IAudioSessionManager2::GetSessionEnumerator: {:08x}", err))
    }

    pub fn get_session_count(
        &self,
        enumerator: &ComPtr<IAudioSessionEnumerator>,
    ) -> Result<i32, Error> {
        let mut count = 0;
        match unsafe { enumerator.GetCount(&mut count) } {
            S_OK => Ok(count),
            err => Err(format_err!(
                "IAudioSessionEnumerator::GetCount: {:08x}",
                err
            )),
        }
    }

    pub fn get_session_control(
        &self,
        enumerator: &ComPtr<IAudioSessionEnumerator>,
        idx: i32,
    ) -> Result<ComPtr<IAudioSessionControl2>, Error> {
        let control = ComPtr::new(|| {
            let mut obj = ptr::null_mut();
            let res = unsafe { enumerator.GetSession(idx, &mut obj) };
            hresult(obj as *mut IAudioSessionControl, res)
        })
        .map_err(|err| format_err!("IAudioSessionEnumerator::GetSession: {:08x}", err))?;
        control
            .query_interface::<IAudioSessionControl2>()
            .map_err(|err| format_err!("IAudioSessionControl2 Error: {:08x}", err))
    }

    pub fn get_session_pid(&self, control: &ComPtr<IAudioSessionControl2>) -> Result<DWORD, Error> {
        let mut process_id: DWORD = 0;
        match unsafe { control.GetProcessId(&mut process_id) } {
            S_OK | AUDCLNT_S_NO_SINGLE_PROCESS => Ok(process_id),
            err => Err(format_err!(
                "IAudioSessionControl2::GetProcessId: {:08x}",
                err
            )),
        }
    }

    pub fn set_session_control(
        &mut self,
        control: &ComPtr<IAudioSessionControl2>,
    ) -> Result<(), Error> {
        let session_control = control
            .query_interface::<ISimpleAudioVolume>()
            .map_err(|err| format_err!("ISimpleAudioVolume: {:08x}", err))?;
        self.control = Some(session_control);
        Ok(())
    }

    pub fn set_mute(&self, state: i32) -> Result<(), Error> {
        match unsafe { self.control()?.SetMute(state, ptr::null()) } {
            S_OK => Ok(()),
            err => Err(format_err!("ISimpleAudioVolume::SetMute: {:08x}", err)),
        }
    }

    fn control(&self) -> Result<ComPtr<ISimpleAudioVolume>, Error> {
        if let Some(control) = &self.control {
            Ok(control.clone())
        } else {
            Err(format_err!("ComIsBleh control isn't set"))
        }
    }
}

impl Drop for ComIsBleh {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}
