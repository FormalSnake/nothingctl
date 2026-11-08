use super::{Connection, DeviceInfo};

const UNSUPPORTED: &str = "the Bluetooth transport needs Linux (BlueZ) or Windows";

pub async fn list() -> anyhow::Result<Vec<DeviceInfo>> {
    anyhow::bail!(UNSUPPORTED)
}

pub async fn open(_address: Option<&str>) -> anyhow::Result<Connection> {
    anyhow::bail!(UNSUPPORTED)
}
