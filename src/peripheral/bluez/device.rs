use crate::peripheral::bluez::Connection;
use dbus::arg::{ArgType, RefArg, Variant};
use dbus::Path;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManufacturerCompany {
    Apple = 0x004C,
    Unknown,
}

impl Default for ManufacturerCompany {
    fn default() -> Self {
        ManufacturerCompany::Unknown
    }
}

impl From<u16> for ManufacturerCompany {
    fn from(value: u16) -> Self {
        match value {
            0x004C => ManufacturerCompany::Apple,
            _ => ManufacturerCompany::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ManufacturerData {
    company: ManufacturerCompany,
    data: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct DeviceProperties {
    services_resolved: bool,
    manufacturer_data: ManufacturerData,
    blocked: bool,
    adapter: String,
    rssi: i64,
    name: String,
    address: String,
    paired: bool,
    icon: String,
    alias: String,
    trusted: bool,
    address_type: String,
    class: u64,
    uuids: Vec<uuid::Uuid>,
    legacy_pairing: bool,
    connected: bool,
}

impl From<HashMap<String, Variant<Box<dyn RefArg>>>> for DeviceProperties {
    fn from(mut value: HashMap<String, Variant<Box<dyn RefArg>>>) -> Self {
        let mut props = Self::default();
        if let Some(data) = value.remove("ServicesResolved").take() {
            props.services_resolved = data.as_u64().unwrap() != 0;
        }

        if let Some(data) = value.remove("ManufacturerData").take() {
            let (mfid, mfdata): (u16, Vec<u8>) = data
                .as_iter()
                .unwrap()
                .next()
                .unwrap()
                .as_iter()
                .unwrap()
                .fold((0u16, Vec::new()), |mut acc, pair| {
                    match pair.arg_type() {
                        ArgType::UInt16 => {
                            acc.0 = pair.as_u64().unwrap() as u16;
                        }
                        ArgType::Variant => {
                            let res: Vec<u8> = pair
                                .as_iter()
                                .unwrap()
                                .next()
                                .unwrap()
                                .as_iter()
                                .unwrap()
                                .fold(Vec::new(), |mut acc, value| {
                                    acc.push(value.as_u64().unwrap() as u8);
                                    acc
                                });

                            acc.1 = res;
                        }
                        _ => {}
                    }

                    acc
                });

            props.manufacturer_data = ManufacturerData {
                data: mfdata,
                company: mfid.into(),
            };
        }

        if let Some(data) = value.remove("Blocked").take() {
            props.blocked = data.as_u64().unwrap() != 0;
        }

        if let Some(data) = value.remove("Path").take() {
            props.adapter = data.as_str().unwrap().into();
        }

        if let Some(data) = value.remove("RSSI").take() {
            props.rssi = data.as_i64().unwrap();
        }

        if let Some(data) = value.remove("Adapter").take() {
            props.adapter = data.as_str().unwrap().into();
        }

        if let Some(data) = value.remove("Name").take() {
            props.name = data.as_str().unwrap().into();
        }

        if let Some(data) = value.remove("Address").take() {
            props.address = data.as_str().unwrap().into()
        }

        if let Some(data) = value.remove("Paired").take() {
            props.paired = data.as_u64().unwrap() != 0;
        }

        if let Some(data) = value.remove("Icon").take() {
            props.icon = data.as_str().unwrap().into();
        }

        if let Some(data) = value.remove("Alias").take() {
            props.alias = data.as_str().unwrap().into();
        }

        if let Some(data) = value.remove("Trusted").take() {
            props.trusted = data.as_u64().unwrap() != 0;
        }

        if let Some(data) = value.remove("AddressType").take() {
            props.address_type = data.as_str().unwrap().into();
        }

        if let Some(data) = value.remove("Class").take() {
            props.class = data.as_u64().unwrap();
        }

        if let Some(data) = value.remove("UUIDs").take() {
            let uuids = data
                .as_iter()
                .unwrap()
                .next()
                .unwrap()
                .as_iter()
                .unwrap()
                .try_fold(Vec::<uuid::Uuid>::new(), |mut acc, device_uuid| {
                    let str_uuid = device_uuid.as_str().unwrap();
                    match uuid::Uuid::parse_str(str_uuid) {
                        Ok(uuid) => {
                            acc.push(uuid);
                            Ok(acc)
                        }
                        Err(e) => Err(e),
                    }
                })
                .unwrap();
            props.uuids = uuids;
        }

        if let Some(data) = value.remove("LegacyPairing").take() {
            props.legacy_pairing = data.as_u64().unwrap() != 0;
        }

        if let Some(data) = value.remove("Connected").take() {
            props.connected = data.as_u64().unwrap() != 0;
        }

        props
    }
}

#[derive(Debug)]
pub struct Device {
    pub object_path: Path<'static>,
    connection: Arc<Connection>,
    properties: Option<Arc<RwLock<DeviceProperties>>>,
}

impl Device {
    pub fn new(connection: Arc<Connection>, path: Path<'static>) -> Self {
        Device {
            connection,
            object_path: path,
            properties: None,
        }
    }

    pub fn assign_properties(&mut self, data: HashMap<String, Variant<Box<RefArg>>>) {
        self.properties = Some(Arc::new(RwLock::new(data.into())));
    }

    pub fn refresh(&self) {
        // TODO: Refresh stuff
    }
}
