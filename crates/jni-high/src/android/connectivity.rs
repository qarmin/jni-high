use jni::objects::{JObject, JValue};
use jni::{jni_sig, jni_str};

use crate::AndroidContext;
use crate::error::{BridgeError, BridgeResult};

/// Broad categorization of the active network interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkType {
    /// No active network connection.
    None,
    /// Connected via Wi-Fi (includes Wi-Fi Aware).
    Wifi,
    /// Connected via mobile data (cellular).
    Mobile,
    /// Connected via Ethernet.
    Ethernet,
    /// Connected via VPN (often active even when physical radios are off).
    Vpn,
    /// Connected via some other transport (Bluetooth, USB, LoWPAN, …).
    Other,
}

/// Returns `true` when there is an active network connection of any kind.
///
/// Requires API 23+ and `android.permission.ACCESS_NETWORK_STATE` in the manifest.
pub fn is_connected() -> BridgeResult<bool> {
    Ok(active_type()? != NetworkType::None)
}

/// Returns the type of the currently active network.
///
/// Requires API 23+ and `android.permission.ACCESS_NETWORK_STATE` in the manifest.
pub fn active_type() -> BridgeResult<NetworkType> {
    let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
    ctx.attach(|env, activity| {
        let svc: JObject = env.new_string("connectivity")?.into();
        let cm: JObject = env
            .call_method(
                activity,
                jni_str!("getSystemService"),
                jni_sig!((name: java.lang.String) -> java.lang.Object),
                &[JValue::Object(&svc)],
            )?
            .l()?;
        if cm.is_null() {
            return Ok(NetworkType::None);
        }
        let network: JObject = env
            .call_method(
                &cm,
                jni_str!("getActiveNetwork"),
                jni_sig!(() -> android.net.Network),
                &[],
            )?
            .l()?;
        if network.is_null() {
            return Ok(NetworkType::None);
        }
        let caps: JObject = env
            .call_method(
                &cm,
                jni_str!("getNetworkCapabilities"),
                jni_sig!((network: android.net.Network) -> android.net.NetworkCapabilities),
                &[JValue::Object(&network)],
            )?
            .l()?;
        if caps.is_null() {
            return Ok(NetworkType::None);
        }
        // NetworkCapabilities.TRANSPORT_* constants
        const TRANSPORT_CELLULAR: i32 = 0;
        const TRANSPORT_WIFI: i32 = 1;
        const TRANSPORT_ETHERNET: i32 = 3;
        const TRANSPORT_VPN: i32 = 4;

        let mut has = |transport: i32| -> BridgeResult<bool> {
            Ok(env
                .call_method(
                    &caps,
                    jni_str!("hasTransport"),
                    jni_sig!((transport: int) -> boolean),
                    &[JValue::Int(transport)],
                )?
                .z()?)
        };

        if has(TRANSPORT_WIFI)? {
            Ok(NetworkType::Wifi)
        } else if has(TRANSPORT_CELLULAR)? {
            Ok(NetworkType::Mobile)
        } else if has(TRANSPORT_ETHERNET)? {
            Ok(NetworkType::Ethernet)
        } else if has(TRANSPORT_VPN)? {
            Ok(NetworkType::Vpn)
        } else {
            Ok(NetworkType::Other)
        }
    })
}
