//! The Bluetooth side: BlueZ on Linux, a stub that errors everywhere else.

use tokio::io::{AsyncRead, AsyncWrite};

use crate::protocol::model::Model;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{list, open};

#[cfg(not(target_os = "linux"))]
mod stub;
#[cfg(not(target_os = "linux"))]
pub use stub::{list, open};

#[derive(Clone, Debug, serde::Serialize)]
pub struct DeviceInfo {
    pub address: String,
    pub name: String,
    pub connected: bool,
}

pub trait IoStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> IoStream for T {}

pub struct Connection {
    pub device: DeviceInfo,
    pub model: &'static Model,
    pub stream: Box<dyn IoStream>,
    /// Keeps the BlueZ session and profile registration alive for as long as
    /// the stream is in use.
    pub _guard: Box<dyn Send>,
}

/// The device answered with a model id that has no verified table, or offers
/// no Fast Pair service to read one from. Nothing was sent to it.
#[derive(Debug)]
pub struct UnsupportedModel {
    pub model_id: Option<String>,
    pub address: String,
}

impl std::fmt::Display for UnsupportedModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let supported = crate::protocol::model::MODELS
            .iter()
            .map(|m| format!("{} {}", m.base, m.name))
            .collect::<Vec<_>>()
            .join(", ");
        match &self.model_id {
            Some(id) => write!(
                f,
                "model id {id} on {} is not supported (supported: {supported}); nothing was sent",
                self.address
            ),
            None => write!(
                f,
                "{} lists no Fast Pair service, so its model cannot be identified (supported: {supported}); nothing was sent",
                self.address
            ),
        }
    }
}

impl std::error::Error for UnsupportedModel {}

/// Picks the device: the given address, or the only connected one.
pub fn select(devices: Vec<DeviceInfo>, address: Option<&str>) -> anyhow::Result<DeviceInfo> {
    if let Some(addr) = address {
        return devices
            .into_iter()
            .find(|d| d.address.eq_ignore_ascii_case(addr))
            .ok_or_else(|| anyhow::anyhow!("{addr} is not a paired Nothing device (see `nothingctl list`)"));
    }
    let mut connected: Vec<DeviceInfo> = devices.into_iter().filter(|d| d.connected).collect();
    match connected.len() {
        0 => anyhow::bail!("no Nothing device is connected"),
        1 => Ok(connected.remove(0)),
        n => anyhow::bail!("{n} Nothing devices are connected, pick one with -d"),
    }
}
