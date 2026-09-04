//! Discover Neon Companion phones over mDNS / DNS-SD.
//!
//! The Companion app advertises its REST API as
//! `PI monitor:<phone name>:<phone hardware id>._http._tcp.local.`, so a plain
//! service browse yields every phone on the LAN together with its hardware id —
//! no LSL streaming required and nothing to rename. This is the identity the
//! bridge pins a station to. `neon.local`, by contrast, is first-come-first-
//! served: with two phones on one lab network it points at whichever answered
//! first, which is how two stations ended up driving the same phone.

use super::DeviceError;
use mdns_sd::{IfKind, ServiceDaemon, ServiceEvent};
use serde::Serialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// DNS-SD service type the Companion app registers under.
pub const NEON_SERVICE_TYPE: &str = "_http._tcp.local.";
/// Instance-name prefix that identifies a Pupil Labs Companion advertisement.
const NEON_INSTANCE_PREFIX: &str = "PI monitor:";

/// A Neon Companion phone seen on the network.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NeonPhone {
    /// Display name configured in the Companion app (defaults to "Neon Companion").
    pub device_name: String,
    /// Phone hardware id — the same value `GET /api/status` reports as `phone.device_id`.
    pub device_id: String,
    pub ip: String,
    pub port: u16,
    /// mDNS hostname, e.g. `neon.local.`
    pub hostname: String,
}

impl NeonPhone {
    /// `ip:port` in the form `PupilDevice::new` accepts.
    pub fn host(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }
}

/// Parse `PI monitor:<name>:<hardware id>[._http._tcp.local.]` into `(name, id)`.
///
/// The name itself may contain colons, so the id is taken from the *last*
/// colon; anything else that is not a Companion advertisement yields `None`.
pub fn parse_neon_instance(fullname: &str) -> Option<(String, String)> {
    let suffix = format!(".{}", NEON_SERVICE_TYPE);
    let instance = fullname.strip_suffix(&suffix).unwrap_or(fullname);
    let rest = instance.strip_prefix(NEON_INSTANCE_PREFIX)?;
    let (name, id) = rest.rsplit_once(':')?;
    if name.is_empty() || id.is_empty() {
        return None;
    }
    Some((name.to_string(), id.to_string()))
}

/// Browse for Neon phones for up to `timeout`. Returns every phone seen,
/// de-duplicated by hardware id.
pub async fn discover_neon_phones(timeout: Duration) -> Result<Vec<NeonPhone>, DeviceError> {
    browse(timeout, None).await.map(|m| {
        let mut phones: Vec<NeonPhone> = m.into_values().collect();
        phones.sort_by(|a, b| {
            a.device_name
                .cmp(&b.device_name)
                .then(a.device_id.cmp(&b.device_id))
        });
        phones
    })
}

/// Look for one specific phone by hardware id, returning as soon as it answers
/// (or `None` after `timeout`).
pub async fn find_neon_phone(
    device_id: &str,
    timeout: Duration,
) -> Result<Option<NeonPhone>, DeviceError> {
    let mut found = browse(timeout, Some(device_id)).await?;
    Ok(found.remove(device_id))
}

async fn browse(
    timeout: Duration,
    stop_at_id: Option<&str>,
) -> Result<HashMap<String, NeonPhone>, DeviceError> {
    let daemon = ServiceDaemon::new().map_err(|e| {
        DeviceError::CommunicationError(format!("mDNS daemon failed to start: {e}"))
    })?;
    // Link-local IPv6 answers are not usable by the plain HTTP client anyway.
    if let Err(e) = daemon.disable_interface(IfKind::IPv6) {
        debug!(
            device = "pupil",
            "Could not disable IPv6 for mDNS browse: {e}"
        );
    }
    let receiver = daemon
        .browse(NEON_SERVICE_TYPE)
        .map_err(|e| DeviceError::CommunicationError(format!("mDNS browse failed: {e}")))?;

    let deadline = Instant::now() + timeout;
    let mut phones: HashMap<String, NeonPhone> = HashMap::new();

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, receiver.recv_async()).await {
            Ok(Ok(ServiceEvent::ServiceResolved(service))) => {
                let Some((device_name, device_id)) = parse_neon_instance(service.get_fullname())
                else {
                    continue;
                };
                let Some(ip) = service.get_addresses_v4().into_iter().next() else {
                    warn!(
                        device = "pupil",
                        device_id = %device_id,
                        "Neon phone advertised without an IPv4 address; skipping"
                    );
                    continue;
                };
                let phone = NeonPhone {
                    device_name,
                    device_id: device_id.clone(),
                    ip: ip.to_string(),
                    port: service.get_port(),
                    hostname: service.get_hostname().to_string(),
                };
                debug!(device = "pupil", ?phone, "Neon phone resolved via mDNS");
                phones.insert(device_id.clone(), phone);
                if stop_at_id == Some(device_id.as_str()) {
                    break;
                }
            }
            Ok(Ok(_)) => {}
            // Channel closed or browse window elapsed.
            Ok(Err(_)) | Err(_) => break,
        }
    }

    let _ = daemon.stop_browse(NEON_SERVICE_TYPE);
    let _ = daemon.shutdown();

    info!(
        device = "pupil",
        count = phones.len(),
        "Neon mDNS discovery finished: {}",
        phones
            .values()
            .map(|p| format!("{} ({}) @ {}", p.device_name, p.device_id, p.ip))
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(phones)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_companion_instance_with_service_suffix() {
        let parsed =
            parse_neon_instance("PI monitor:Neon Companion:a41fe4fe2bccf6c3._http._tcp.local.");
        assert_eq!(
            parsed,
            Some(("Neon Companion".to_string(), "a41fe4fe2bccf6c3".to_string()))
        );
    }

    #[test]
    fn parses_instance_without_suffix_and_keeps_colons_in_name() {
        let parsed = parse_neon_instance("PI monitor:Lab: P1 phone:3a7a373396c1afc4");
        assert_eq!(
            parsed,
            Some(("Lab: P1 phone".to_string(), "3a7a373396c1afc4".to_string()))
        );
    }

    #[test]
    fn ignores_other_http_services() {
        assert_eq!(
            parse_neon_instance("Brother HL-2270DW._http._tcp.local."),
            None
        );
        assert_eq!(
            parse_neon_instance("PI monitor:onlyname._http._tcp.local."),
            None
        );
        assert_eq!(
            parse_neon_instance("PI monitor::id._http._tcp.local."),
            None
        );
    }

    #[test]
    fn host_joins_ip_and_port() {
        let phone = NeonPhone {
            device_name: "Neon P1".into(),
            device_id: "abc".into(),
            ip: "192.168.50.220".into(),
            port: 8080,
            hostname: "neon.local.".into(),
        };
        assert_eq!(phone.host(), "192.168.50.220:8080");
    }
}
