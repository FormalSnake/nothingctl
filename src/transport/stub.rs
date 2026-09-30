use super::{Connection, DeviceInfo};

const NEEDS_LINUX: &str = "the Bluetooth transport needs Linux and BlueZ";

pub async fn list() -> anyhow::Result<Vec<DeviceInfo>> {
    anyhow::bail!(NEEDS_LINUX)
}

pub async fn open(_address: Option<&str>) -> anyhow::Result<Connection> {
    anyhow::bail!(NEEDS_LINUX)
}
