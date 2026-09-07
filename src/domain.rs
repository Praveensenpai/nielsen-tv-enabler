//! Domain logic for Nielsen Accessibility and Prompt handling.

pub mod detect;
pub mod device;
pub mod prompt;
pub mod service;
pub mod sync;
pub mod vpn;

pub use detect::detect_nielsen_service;
pub use device::DeviceCommander;
