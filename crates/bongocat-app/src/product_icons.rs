//! The status and Dock icons, as the settings worker's capabilities.
//!
//! Both sit behind a request channel rather than a direct platform call: the
//! icon belongs to the GPUI thread that owns the window, while the settings worker
//! is the one asking for the change, so the request carries the answer back.

use super::*;

pub(crate) struct StatusIconRequest {
    pub(crate) visible: bool,
    pub(crate) reply: std::sync::mpsc::SyncSender<Result<(), SettingsError>>,
}

#[derive(Clone)]
pub(crate) struct ProductStatusIcon {
    pub(crate) sender: std::sync::mpsc::SyncSender<StatusIconRequest>,
}

impl bongocat_app::StatusIconCapability for ProductStatusIcon {
    fn set_visible(&self, visible: bool) -> Result<(), SettingsError> {
        let (reply, receiver) = std::sync::mpsc::sync_channel(1);
        self.sender
            .try_send(StatusIconRequest { visible, reply })
            .map_err(|_| SettingsError::new(SettingsErrorCode::StatusIconUpdateFailed))?;
        receiver
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| SettingsError::new(SettingsErrorCode::StatusIconUpdateFailed))?
    }
}

pub(crate) struct DockIconRequest {
    pub(crate) visible: bool,
    pub(crate) reply: std::sync::mpsc::SyncSender<Result<(), SettingsError>>,
}

#[derive(Clone)]
pub(crate) struct ProductDockIcon {
    pub(crate) sender: std::sync::mpsc::SyncSender<DockIconRequest>,
}

impl bongocat_app::DockIconCapability for ProductDockIcon {
    fn set_visible(&self, visible: bool) -> Result<(), SettingsError> {
        let (reply, receiver) = std::sync::mpsc::sync_channel(1);
        self.sender
            .try_send(DockIconRequest { visible, reply })
            .map_err(|_| SettingsError::new(SettingsErrorCode::DockIconUpdateFailed))?;
        receiver
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| SettingsError::new(SettingsErrorCode::DockIconUpdateFailed))?
    }
}
