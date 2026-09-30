//! BlueZ transport. Every RFCOMM link is opened by service UUID: a client
//! profile is registered for the UUID and the device's ConnectProfile makes
//! BlueZ resolve the channel over SDP and hand over the socket. No channel
//! number appears anywhere.

use std::time::Duration;

use anyhow::{Context, anyhow, bail};
use bluer::rfcomm::{Profile, ProfileHandle, ReqError, Role, Stream};
use bluer::{Address, Device, Session, Uuid};
use futures::StreamExt;
use tokio::io::AsyncReadExt;

use super::{Connection, DeviceInfo, select};
use crate::protocol::frame::hex;
use crate::protocol::model::{self, CONTROL_UUID, FAST_PAIR_UUID};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const MODEL_ID_TIMEOUT: Duration = Duration::from_secs(5);

fn uuid(s: &str) -> Uuid {
    s.parse().expect("constant uuid")
}

async fn info(device: &Device) -> bluer::Result<DeviceInfo> {
    let name = match device.alias().await {
        Ok(a) => a,
        Err(_) => device.name().await?.unwrap_or_default(),
    };
    Ok(DeviceInfo { address: device.address().to_string(), name, connected: device.is_connected().await? })
}

async fn matching(session: &Session) -> anyhow::Result<Vec<(Device, DeviceInfo)>> {
    let adapter = session.default_adapter().await.context("no Bluetooth adapter")?;
    let control = uuid(CONTROL_UUID);
    let mut out = Vec::new();
    for addr in adapter.device_addresses().await? {
        let device = adapter.device(addr)?;
        if !device.is_paired().await.unwrap_or(false) {
            continue;
        }
        let has_control = device.uuids().await.ok().flatten().is_some_and(|u| u.contains(&control));
        if has_control {
            let i = info(&device).await?;
            out.push((device, i));
        }
    }
    Ok(out)
}

pub async fn list() -> anyhow::Result<Vec<DeviceInfo>> {
    let session = Session::new().await.context("cannot reach BlueZ over D-Bus")?;
    Ok(matching(&session).await?.into_iter().map(|(_, i)| i).collect())
}

/// Registers a client profile for `service` and asks BlueZ to connect it.
async fn connect_service(session: &Session, device: &Device, service: Uuid) -> anyhow::Result<(Stream, ProfileHandle)> {
    let mut handle = session
        .register_profile(Profile {
            uuid: service,
            role: Some(Role::Client),
            auto_connect: Some(false),
            ..Default::default()
        })
        .await
        .with_context(|| format!("BlueZ refused a client profile for {service}"))?;
    let addr: Address = device.address();
    let connect = device.connect_profile(&service);
    tokio::pin!(connect);
    let mut connect_done = false;
    let deadline = tokio::time::sleep(CONNECT_TIMEOUT);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            res = &mut connect, if !connect_done => {
                connect_done = true;
                if let Err(e) = res {
                    bail!("BlueZ could not connect service {service} on {addr} ({e}); nothing was sent");
                }
            }
            req = handle.next() => {
                let req = req.ok_or_else(|| anyhow!("BlueZ dropped the profile for {service}"))?;
                if req.device() == addr {
                    let stream = req.accept().context("accepting the RFCOMM socket")?;
                    return Ok((stream, handle));
                }
                req.reject(ReqError::Rejected);
            }
            _ = &mut deadline => bail!("timed out connecting service {service} on {addr}; nothing was sent"),
        }
    }
}

/// ear-web's model query: read the Fast Pair channel until a chunk of
/// exactly 7 bytes arrives and take bytes 4..6 as the model id. Nothing is
/// written to this channel.
async fn read_model_id(session: &Session, device: &Device) -> anyhow::Result<String> {
    let (stream, handle) = connect_service(session, device, uuid(FAST_PAIR_UUID)).await?;
    let (mut reader, _unused_writer) = tokio::io::split(stream);
    let mut buf = [0u8; 256];
    let id = tokio::time::timeout(MODEL_ID_TIMEOUT, async {
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
    .map_err(|_| anyhow!("no model id on the Fast Pair channel within {}s", MODEL_ID_TIMEOUT.as_secs()))??;
    drop(handle);
    Ok(id)
}

pub async fn open(address: Option<&str>) -> anyhow::Result<Connection> {
    let session = Session::new().await.context("cannot reach BlueZ over D-Bus")?;
    let devices = matching(&session).await?;
    let chosen = select(devices.iter().map(|(_, i)| i.clone()).collect(), address)?;
    let device = devices
        .into_iter()
        .find(|(_, i)| i.address == chosen.address)
        .map(|(d, _)| d)
        .ok_or_else(|| anyhow!("{} disappeared", chosen.address))?;

    let has_fast_pair = device.uuids().await.ok().flatten().is_some_and(|u| u.contains(&uuid(FAST_PAIR_UUID)));
    if !has_fast_pair {
        bail!("{} lists no Fast Pair service, so its model cannot be identified; nothing was sent", chosen.address);
    }
    let model_id = read_model_id(&session, &device).await?;
    crate::debug!("model id {model_id}");
    let model = model::by_model_id(&model_id).ok_or_else(|| {
        anyhow!("model id {model_id} is not supported (supported: {}); nothing was sent", supported())
    })?;

    let (stream, handle) = connect_service(&session, &device, uuid(CONTROL_UUID)).await?;
    Ok(Connection { device: chosen, model, stream: Box::new(stream), _guard: Box::new((handle, session)) })
}

fn supported() -> String {
    model::MODELS.iter().map(|m| format!("{} {}", m.base, m.name)).collect::<Vec<_>>().join(", ")
}
