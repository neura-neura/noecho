use thiserror::Error;

pub type Result<T> = std::result::Result<T, AudioError>;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("{0}")]
    Message(String),

    #[error("COM error 0x{0:08X}: {1}")]
    Com(u32, String),

    #[error("device not found: {0}")]
    DeviceNotFound(String),

    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("protection is already active")]
    ProtectionAlreadyActive,

    #[error("protection is not active")]
    ProtectionNotActive,

    #[error("shared output device is not available: {0}")]
    SharedDeviceUnavailable(String),

    #[error("physical output device is not available: {0}")]
    PhysicalDeviceUnavailable(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl AudioError {
    pub fn message(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }

    pub fn from_hresult(hr: windows::core::HRESULT, context: &str) -> Self {
        let code = hr.0 as u32;
        Self::Com(code, format!("{context} (HRESULT 0x{code:08X})"))
    }
}

impl From<windows::core::Error> for AudioError {
    fn from(value: windows::core::Error) -> Self {
        let code = value.code().0 as u32;
        Self::Com(code, value.to_string())
    }
}
