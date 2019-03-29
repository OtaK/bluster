use crate::peripheral::bluez::{
    constants::{BLUEZ_SERVICE_NAME, DBUS_PROPERTIES_IFACE, DEVICE_IFACE, NETWORK_IFACE},
    Connection,
};
use crate::Error;
use dbus::arg::{ArgType, RefArg, Variant};
use dbus::stdintf::org_freedesktop_dbus::Properties;
use dbus::{Message, Path};
use futures::prelude::*;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DevicePanStatus {
    Disconnected,
    Connecting,
    Connected(String),
}

impl Default for DevicePanStatus {
    fn default() -> Self {
        DevicePanStatus::Disconnected
    }
}

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
    pub services_resolved: bool,
    pub manufacturer_data: ManufacturerData,
    pub blocked: bool,
    pub adapter: String,
    pub rssi: i16,
    pub name: String,
    pub address: String,
    pub paired: bool,
    pub icon: String,
    pub alias: String,
    pub trusted: bool,
    pub address_type: String,
    pub class: u64,
    pub uuids: Vec<uuid::Uuid>,
    pub legacy_pairing: bool,
    pub connected: bool,
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
            props.rssi = data.as_i64().unwrap() as i16;
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
    pan_status: Arc<RwLock<DevicePanStatus>>,
    properties: Arc<RwLock<DeviceProperties>>,
}

impl std::ops::Deref for Device {
    type Target = Arc<RwLock<DeviceProperties>>;
    fn deref(&self) -> &Self::Target {
        &self.properties
    }
}

impl Device {
    pub fn new(connection: Arc<Connection>, path: Path<'static>) -> Self {
        Device {
            connection,
            object_path: path,
            pan_status: Arc::new(RwLock::new(DevicePanStatus::default())),
            properties: Arc::new(RwLock::new(DeviceProperties::default())),
        }
    }

    pub fn assign_properties(&mut self, data: HashMap<String, Variant<Box<RefArg>>>) {
        *self.properties.write() = data.into();
    }

    pub fn refresh(&self) {
        let message = Message::new_method_call(
            BLUEZ_SERVICE_NAME,
            self.object_path.clone(),
            DBUS_PROPERTIES_IFACE,
            "GetAll",
        )
        .unwrap();

        let inner_properties = Arc::clone(&self.properties);

        let method_call = self
            .connection
            .default
            .method_call(message)
            .unwrap()
            .from_err()
            .and_then(|reply| {
                reply
                    .read1::<HashMap<String, Variant<Box<dyn RefArg>>>>()
                    .map_err(Error::from)
            })
            .map(DeviceProperties::from)
            .and_then(move |new_props| {
                *inner_properties.write() = new_props;

                Ok(())
            })
            .map_err(|_| ());

        self.connection.runtime.lock().unwrap().spawn(method_call);
    }

    pub fn update_rssi(&self) {
        let props =
            self.connection
                .fallback
                .with_path(BLUEZ_SERVICE_NAME, self.object_path.clone(), 5000);

        let maybe_rssi: Result<i16, dbus::Error> = props.get(DEVICE_IFACE, "RSSI");
        match maybe_rssi {
            Ok(rssi) => {
                self.properties.write().rssi = rssi;
            }
            Err(error) => {
                println!("{}", error);
            }
        }
    }

    pub fn connect_pan(&self) -> impl Future<Item = (), Error = Error> {
        *self.pan_status.write() = DevicePanStatus::Connecting;
        let message = Message::new_method_call(
            BLUEZ_SERVICE_NAME,
            self.object_path.clone(),
            NETWORK_IFACE,
            "Connect",
        )
        .unwrap()
        .append("nap");

        let method_call = self
            .connection
            .default
            .method_call(message)
            .unwrap()
            .from_err()
            .and_then(|reply| reply.read1::<String>().map_err(Error::from));

        let conn_status = Arc::clone(&self.pan_status);
        method_call.and_then(move |connection_id| {
            *conn_status.write() = DevicePanStatus::Connected(connection_id);
            Ok(())
        })
    }

    pub fn disconnect_pan(&self) -> impl Future<Item = (), Error = Error> {
        let message = Message::new_method_call(
            BLUEZ_SERVICE_NAME,
            self.object_path.clone(),
            NETWORK_IFACE,
            "Disconnect",
        )
        .unwrap();

        let message_call = self
            .connection
            .default
            .method_call(message)
            .unwrap()
            .from_err();

        let conn_status = Arc::clone(&self.pan_status);
        message_call.and_then(move |_| {
            *conn_status.write() = DevicePanStatus::Disconnected;
            Ok(())
        })
    }

    pub fn refresh_pan_status(&self) -> Result<DevicePanStatus, Error> {
        let props =
            self.connection
                .fallback
                .with_path(BLUEZ_SERVICE_NAME, self.object_path.clone(), 5000);

        let connected: bool = props.get(NETWORK_IFACE, "Connected")?;
        let status = if connected {
            let interface: String = props.get(NETWORK_IFACE, "Interface")?;
            DevicePanStatus::Connected(interface)
        } else {
            DevicePanStatus::Disconnected
        };

        *self.pan_status.write() = status.clone();

        Ok(status)
    }
}
