use windows::Win32::System::Com::{
    CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED,
};

/// RAII COM initializer for the current thread.
pub struct ComApartment {
    initialized_here: bool,
}

impl ComApartment {
    pub fn init_mta() -> windows::core::Result<Self> {
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        // S_FALSE means COM was already initialized on this thread.
        if hr.is_ok() || hr == windows::core::HRESULT(1i32) /* S_FALSE */ {
            Ok(Self {
                initialized_here: hr.is_ok() && hr.0 == 0,
            })
        } else if hr.0 as u32 == 0x80010106 {
            // RPC_E_CHANGED_MODE — already initialized with different model.
            Ok(Self {
                initialized_here: false,
            })
        } else {
            Err(hr.into())
        }
    }

    pub fn init_sta() -> windows::core::Result<Self> {
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if hr.is_ok() || hr == windows::core::HRESULT(1i32) {
            Ok(Self {
                initialized_here: hr.is_ok() && hr.0 == 0,
            })
        } else if hr.0 as u32 == 0x80010106 {
            Ok(Self {
                initialized_here: false,
            })
        } else {
            Err(hr.into())
        }
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.initialized_here {
            unsafe {
                CoUninitialize();
            }
        }
    }
}
