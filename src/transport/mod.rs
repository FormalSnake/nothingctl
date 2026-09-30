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
