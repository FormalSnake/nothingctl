//! Windows transport. Devices and their SDP records come from WinRT; the
//! RFCOMM link is a Winsock AF_BTH socket addressed by service class UUID with
//! port 0, so Windows resolves the channel over SDP. No channel number appears
//! anywhere.

use std::io;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll, ready};

use anyhow::{Context, anyhow};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc;
use windows::Devices::Bluetooth::Rfcomm::RfcommServiceId;
use windows::Devices::Bluetooth::{BluetoothCacheMode, BluetoothConnectionStatus, BluetoothDevice};
use windows::Devices::Enumeration::DeviceInformation;
use windows::Win32::Devices::Bluetooth::{AF_BTH, BTHPROTO_RFCOMM, SOCKADDR_BTH};
use windows::Win32::Networking::WinSock::{
    SD_BOTH, SEND_RECV_FLAGS, SOCK_STREAM, SOCKADDR, SOCKET, WSADATA, WSAGetLastError, WSAStartup, closesocket,
    connect, recv, send, shutdown, socket,
};
use windows::core::GUID;

use super::{Connection, DeviceInfo, UnsupportedModel, read_model_id, select};
use crate::protocol::model::{self, CONTROL_UUID, FAST_PAIR_UUID};

fn guid(s: &str) -> GUID {
    GUID::from_u128(u128::from_str_radix(&s.replace('-', ""), 16).expect("constant uuid"))
}

fn format_address(addr: u64) -> String {
    let b = addr.to_be_bytes();
    format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}", b[2], b[3], b[4], b[5], b[6], b[7])
}

/// Whether the device's cached SDP records list `service`. Windows reads them
/// at pairing, so this works while the device is out of range.
fn offers(device: &BluetoothDevice, service: &str) -> windows::core::Result<bool> {
    let id = RfcommServiceId::FromUuid(guid(service))?;
    let result = device.GetRfcommServicesForIdWithCacheModeAsync(&id, BluetoothCacheMode::Cached)?.join()?;
    Ok(result.Services()?.Size()? > 0)
}

fn matching() -> anyhow::Result<Vec<(BluetoothDevice, DeviceInfo)>> {
    let selector = BluetoothDevice::GetDeviceSelectorFromPairingState(true)?;
    let infos = DeviceInformation::FindAllAsyncAqsFilter(&selector)?
        .join()
        .context("cannot enumerate paired Bluetooth devices")?;
    let mut out = Vec::new();
    for info in infos {
        let Ok(device) = BluetoothDevice::FromIdAsync(&info.Id()?)?.join() else { continue };
        if !offers(&device, CONTROL_UUID).unwrap_or(false) {
            continue;
        }
        let i = DeviceInfo {
            address: format_address(device.BluetoothAddress()?),
            name: device.Name()?.to_string(),
            connected: device.ConnectionStatus()? == BluetoothConnectionStatus::Connected,
        };
        out.push((device, i));
    }
    Ok(out)
}

pub async fn list() -> anyhow::Result<Vec<DeviceInfo>> {
    tokio::task::spawn_blocking(|| Ok(matching()?.into_iter().map(|(_, i)| i).collect())).await?
}

struct Socket(SOCKET);

impl Drop for Socket {
    fn drop(&mut self) {
        // Closing unblocks the reader thread's recv, which then exits.
        unsafe {
            shutdown(self.0, SD_BOTH);
            closesocket(self.0);
        }
    }
}

fn wsa_error(what: &str) -> io::Error {
    let code = unsafe { WSAGetLastError() };
    io::Error::new(
        io::Error::from_raw_os_error(code.0).kind(),
        format!("{what}: {}", io::Error::from_raw_os_error(code.0)),
    )
}

fn connect_service(address: u64, service: &str) -> anyhow::Result<Socket> {
    let mut data = WSADATA::default();
    let rc = unsafe { WSAStartup(0x0202, &mut data) };
    if rc != 0 {
        anyhow::bail!("WSAStartup failed ({rc})");
    }
    let sock = unsafe { socket(AF_BTH as i32, SOCK_STREAM, BTHPROTO_RFCOMM as i32) }
        .map_err(|e| anyhow!("cannot open an RFCOMM socket: {e}"))?;
    let sock = Socket(sock);
    let addr = SOCKADDR_BTH { addressFamily: AF_BTH, btAddr: address, serviceClassId: guid(service), port: 0 };
    let rc =
        unsafe { connect(sock.0, &addr as *const SOCKADDR_BTH as *const SOCKADDR, size_of::<SOCKADDR_BTH>() as i32) };
    if rc != 0 {
        let e = wsa_error("connect");
        anyhow::bail!("could not connect service {service} on {} ({e}); nothing was sent", format_address(address));
    }
    Ok(sock)
}

/// A blocking Winsock RFCOMM socket as a tokio stream: a thread blocks in
/// recv and forwards chunks, writes go straight to send. Frames are a few
/// dozen bytes, so send never blocks long enough to matter.
struct BthStream {
    socket: Socket,
    rx: mpsc::UnboundedReceiver<io::Result<Vec<u8>>>,
    pending: Vec<u8>,
}

impl BthStream {
    fn new(socket: Socket) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let raw = socket.0;
        std::thread::spawn(move || {
            let mut buf = [0u8; 1024];
            loop {
                let n = unsafe { recv(raw, &mut buf, SEND_RECV_FLAGS(0)) };
                let msg = match n {
                    0 => break,
                    n if n < 0 => Err(wsa_error("recv")),
                    n => Ok(buf[..n as usize].to_vec()),
                };
                let failed = msg.is_err();
                if tx.send(msg).is_err() || failed {
                    break;
                }
            }
        });
        Self { socket, rx, pending: Vec::new() }
    }
}

impl AsyncRead for BthStream {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>, out: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        if self.pending.is_empty() {
            match ready!(self.rx.poll_recv(cx)) {
                Some(Ok(chunk)) => self.pending = chunk,
                Some(Err(e)) => return Poll::Ready(Err(e)),
                None => return Poll::Ready(Ok(())),
            }
        }
        let n = self.pending.len().min(out.remaining());
        out.put_slice(&self.pending[..n]);
        self.pending.drain(..n);
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for BthStream {
    fn poll_write(self: Pin<&mut Self>, _cx: &mut TaskContext<'_>, data: &[u8]) -> Poll<io::Result<usize>> {
        let n = unsafe { send(self.socket.0, data, SEND_RECV_FLAGS(0)) };
        Poll::Ready(if n < 0 { Err(wsa_error("send")) } else { Ok(n as usize) })
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

async fn open_stream(address: u64, service: &'static str) -> anyhow::Result<BthStream> {
    let socket = tokio::task::spawn_blocking(move || connect_service(address, service)).await??;
    Ok(BthStream::new(socket))
}

pub async fn open(address: Option<&str>) -> anyhow::Result<Connection> {
    let devices = tokio::task::spawn_blocking(matching).await??;
    let chosen = select(devices.iter().map(|(_, i)| i.clone()).collect(), address)?;
    let device = devices
        .into_iter()
        .find(|(_, i)| i.address == chosen.address)
        .map(|(d, _)| d)
        .ok_or_else(|| anyhow!("{} disappeared", chosen.address))?;
    let bt_addr = device.BluetoothAddress()?;

    if !offers(&device, FAST_PAIR_UUID).unwrap_or(false) {
        return Err(UnsupportedModel { model_id: None, address: chosen.address }.into());
    }
    let model_id = read_model_id(open_stream(bt_addr, FAST_PAIR_UUID).await?).await?;
    crate::debug!("model id {model_id}");
    let Some(model) = model::by_model_id(&model_id) else {
        return Err(UnsupportedModel { model_id: Some(model_id), address: chosen.address }.into());
    };

    let stream = open_stream(bt_addr, CONTROL_UUID).await?;
    Ok(Connection { device: chosen, model, model_id, stream: Box::new(stream), _guard: Box::new(()) })
}
