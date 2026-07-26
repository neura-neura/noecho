//! NoEcho audio-core
//!
//! Windows Core Audio integration for universal shared/private routing.

pub mod com;
pub mod config;
pub mod devices;
pub mod error;
pub mod grouping;
pub mod icons;
pub mod loopback;
pub mod persist;
pub mod policy;
pub mod process;
pub mod protection;
pub mod report;
pub mod sessions;
pub mod setup;
pub mod types;

pub use config::{AppConfig, ThemePreference};
pub use devices::{AudioDevice, DeviceService};
pub use error::{AudioError, Result};
pub use grouping::{group_sessions, AppAudioGroup};
pub use persist::{PersistedState, StateStore};
pub use protection::{ProtectionEngine, ProtectionSnapshot, ProtectionStatus};
pub use report::DiagnosticReport;
pub use sessions::{AudioSessionInfo, SessionService};
pub use setup::{PrepareResult, SetupService, SetupStatus, SetupState};
pub use types::{AppIdentity, PlaybackState, ProtectionMode};

/// Library version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Friendly name for the managed shared endpoint.
pub const SHARED_DEVICE_FRIENDLY_NAME: &str = "Audio compartido";

/// Known provisional virtual-device name fragments used during development.
pub const KNOWN_VIRTUAL_DEVICE_HINTS: &[&str] = &[
    "Audio compartido",
    "CABLE Input",
    "CABLE Output",
    "VB-Audio",
    "VB-Cable",
    "Voicemeeter",
    "Virtual Cable",
    "Virtual Audio",
    "NoEcho Shared",
];
