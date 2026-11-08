//! The Bluetooth side: BlueZ on Linux, Winsock on Windows, a stub that errors
//! everywhere else.

use std::time::Duration;

use anyhow::{anyhow, bail};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};

use crate::protocol::frame::hex;
use crate::protocol::model::Model;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{list, open};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{list, open};

#[cfg(not(any(target_os = "linux", windows)))]
mod stub;
#[cfg(not(any(target_os = "linux", windows)))]
pub use stub::{list, open};

const MODEL_ID_TIMEOUT: Duration = Duration::from_secs(5);

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
    pub model_id: String,
    pub stream: Box<dyn IoStream>,
    /// Keeps the platform session (BlueZ profile registration) alive for as
    /// long as the stream is in use.
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

/// ear-web's model query: read the Fast Pair channel until a chunk of
/// exactly 7 bytes arrives and take bytes 4..6 as the model id. Nothing is
/// written to this channel.
async fn read_model_id(mut reader: impl AsyncRead + Unpin) -> anyhow::Result<String> {
    let mut buf = [0u8; 256];
    tokio::time::timeout(MODEL_ID_TIMEOUT, async {
        loop {
            let n = reader.read(&mut buf).await?;
            if n == 0 {
                bail!("the Fast Pair channel closed before the model id arrived");
            }
            crate::debug!("fast pair rx {}", hex(&buf[..n]));
            if n == 7 {
                return Ok(hex(&buf[4..7]).to_uppercase());
            }
        }
    })
    .await
    .map_err(|_| anyhow!("no model id on the Fast Pair channel within {}s", MODEL_ID_TIMEOUT.as_secs()))?
}
