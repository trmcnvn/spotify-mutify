use anyhow::{anyhow, Result};
use com_ptr::ComPtr;
use external::process::ProcessId;
use external::FromInner;
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

type AudioControl = ComPtr<ISimpleAudioVolume>;
pub(crate) struct Windows {
    control: Option<AudioControl>,
}

impl Windows {
    pub fn new() -> Self {
        unsafe { CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED) };
        Self { control: None }
    }

    /// Return the `ISimpleAudioVolume` interface to the Windows volume control for the target `ProcessId`
    pub fn find_audio_control(&mut self, process_id: ProcessId) -> Result<AudioControl> {
        // Get Default Audio Device
        let device_enumerator = com_ptr::co_create_instance::<IMMDeviceEnumerator>(
            &CLSID_MMDeviceEnumerator,
            None,
            CLSCTX_ALL,
        )
        .map_err(|err| anyhow!("co_create_instance::<IMMDeviceEnumerator>: {:08x}", err))?;

        let device = ComPtr::new(|| {
            let mut obj = ptr::null_mut();
            let res = unsafe {
                device_enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia, &mut obj)
            };
            com_ptr::hresult(obj as *mut IMMDevice, res)
        })
        .map_err(|err| anyhow!("IMMDeviceEnumerator::GetDefaultAudioEndpoint: {:08x}", err))?;

        // Get session enumerator from the audio device
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
            com_ptr::hresult(obj as *mut IAudioSessionManager2, res)
        })
        .map_err(|err| anyhow!("IMMDevice::Activate: {:08x}", err))?;

        let session_enumerator = ComPtr::new(|| {
            let mut obj = ptr::null_mut();
            let res = unsafe { session_manager.GetSessionEnumerator(&mut obj) };
            com_ptr::hresult(obj as *mut IAudioSessionEnumerator, res)
        })
        .map_err(|err| anyhow!("IAudioSessionManager2::GetSessionEnumerator: {:08x}", err))?;

        // Get the count of sessions
        let mut count = 0;
        match unsafe { session_enumerator.GetCount(&mut count) } {
            S_OK => {}
            err => return Err(anyhow!("IAudioSessionEnumerator::GetCount: {:08x}", err)),
        };

        // Iterate over each session to find the one owned by Spotify
        for idx in 0..count {
            // Get the control for this session index
            let control = ComPtr::new(|| {
                let mut obj = ptr::null_mut();
                let res = unsafe { session_enumerator.GetSession(idx, &mut obj) };
                com_ptr::hresult(obj as *mut IAudioSessionControl, res)
            })
            .map_err(|err| anyhow!("IAudioSessionEnumerator::GetSession: {:08x}", err))?;
            let control = control
                .query_interface::<IAudioSessionControl2>()
                .map_err(|err| anyhow!("IAudioSessionControl2 Error: {:08x}", err))?;

            // Get process id associated with the control
            let mut session_pid: DWORD = 0;
            match unsafe { control.GetProcessId(&mut session_pid) } {
                S_OK | AUDCLNT_S_NO_SINGLE_PROCESS => {}
                err => return Err(anyhow!("IAudioSessionControl2::GetProcessId: {:08x}", err)),
            };

            // Check if it's owned by Spotify
            if unsafe { ProcessId::from_inner(session_pid) } == process_id {
                let control = control
                    .query_interface::<ISimpleAudioVolume>()
                    .map_err(|err| anyhow!("ISimpleAudioVolume: {:08x}", err))?;
                self.control = Some(control.clone());
                return Ok(control);
            }
        }

        Err(anyhow!("Couldn't find the audio control owned by Spotify"))
    }

    /// Mute/Unmutes the control set by `find_audio_control`
    pub fn set_mute(&self, value: bool) -> Result<()> {
        if let Some(control) = &self.control {
            return match unsafe { control.SetMute(value as i32, ptr::null()) } {
                S_OK => Ok(()),
                err => Err(anyhow!("ISimpleAudioVolume::SetMute: {:08x}", err)),
            };
        }
        Err(anyhow!("Windows audio control wasn't set"))
    }
}

impl Drop for Windows {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}
