use dbus::{
    arg::{messageitem::MessageItem, RefArg, Variant},
    Path,
};
use std::{collections::HashMap, sync::Arc};

use super::{
    connection::Connection,
    constants::{
        ADAPTER_IFACE, DBUS_OBJECTMANAGER_IFACE, DBUS_PROPERTIES_IFACE,
        LE_ADVERTISING_MANAGER_IFACE,
    },
};
use crate::Error;

#[derive(Debug, Clone)]
pub struct Adapter {
    pub object_path: Path<'static>,
    connection: Arc<Connection>,
}

type ManagedObjectsProps =
    HashMap<Path<'static>, HashMap<String, HashMap<String, Variant<Box<dyn RefArg>>>>>;

impl Adapter {
    async fn find_adapter(connection: &Arc<Connection>) -> Result<Path<'static>, Error> {
        let path = "/".into();
        let proxy = connection.get_bluez_proxy(&path);

        let (props,): (ManagedObjectsProps,) = proxy
            .method_call(DBUS_OBJECTMANAGER_IFACE, "GetManagedObjects", ())
            .await?;
        Ok(props
            .into_iter()
            .find(|(_path, props)| props.contains_key(LE_ADVERTISING_MANAGER_IFACE))
            .map(|(path, _props)| path)
            .expect("LEAdvertisingManager1 interface not found"))
    }

    #[allow(clippy::new_ret_no_self)]
    pub async fn new(connection: Arc<Connection>) -> Result<Self, Error> {
        Adapter::find_adapter(&connection)
            .await
            .map(|object_path| Adapter {
                object_path,
                connection,
            })
    }

    pub async fn powered(self: &Self, on: bool) -> Result<(), Error> {
        let proxy = self.connection.get_bluez_proxy(&self.object_path);
        proxy
            .method_call(
                DBUS_PROPERTIES_IFACE,
                "Set",
                (
                    ADAPTER_IFACE,
                    "Powered",
                    MessageItem::Variant(Box::new(on.into())),
                ),
            )
            .await?;
        Ok(())
    }

    pub async fn is_powered(self: &Self) -> Result<bool, Error> {
        let proxy = self.connection.get_bluez_proxy(&self.object_path);
        let (powered,): (Variant<bool>,) = proxy
            .method_call(DBUS_PROPERTIES_IFACE, "Get", (ADAPTER_IFACE, "Powered"))
            .await?;
        Ok(powered.0)
    }

    pub fn devices(
        &self,
    ) -> Box<
        impl Future<
            Item = HashMap<
                Path<'static>,
                HashMap<String, HashMap<String, Variant<Box<RefArg + 'static>>>>,
            >,
            Error = Error,
        >,
    > {
        let message = Message::new_method_call(
            BLUEZ_SERVICE_NAME,
            "/",
            DBUS_OBJECTMANAGER_IFACE,
            "GetManagedObjects",
        )
        .unwrap();

        let map_connection = Arc::clone(&self.connection);

        let method_call = self
            .connection
            .default
            .method_call(message)
            .unwrap()
            .map_err(Error::from)
            .and_then(|reply| {
                reply
                    .read1::<HashMap<Path, HashMap<String, HashMap<String, Variant<Box<RefArg>>>>>>(
                    )
                    .map_err(Error::from)
            })
            .map(move |map| {
                map.into_iter()
                    .filter_map(|(path, props)| {
                        if path.contains("/org/bluez/hci0/dev_")
                            && props.contains_key("org.bluez.Device1")
                        {
                            let spath: String = format!("{}", path);
                            let spath_count = spath.split('/').count();
                            if spath_count == 5 {
                                return Some((path, props));
                            }
                        }

                        None
                    })
                    .fold(HashMap::new(), move |mut acc, (path, mut props)| {
                        let bz_device = props.remove("org.bluez.Device1").unwrap();
                        let p = path.as_str().unwrap().clone().into();
                        let mut device = Device::new(Arc::clone(&map_connection), path);
                        device.assign_properties(bz_device);
                        acc.insert(p, device);
                        acc
                    })
            });

        Box::new(method_call)
    }
}
