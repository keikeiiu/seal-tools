//! Port enumeration: what the OS sees, reduced to the fields the choice needs.
//!
//! v2's `Arduino.Find` had two halves (`Arduino.cs:25-76`): match the configured USB VID/PID, and —
//! when nothing matches — fall back to the first port whose device NAME mentions "Arduino". The
//! second half exists for a machine whose IDs are not reported correctly, which is exactly the
//! machine where losing the fallback hurts most. Both halves are decisions over data, so both are
//! pure here and tested against hand-built lists; only [`available_ports`] calls the OS.
//!
//! The choice itself stays [`crate::port_choice::choose`]: this module only builds its input. A
//! named `arduino.port` that is present wins; one that is absent is refused rather than replaced by
//! the discovered board (the direction that matters).
//!
//! **The untested line** is `serialport::available_ports()` itself: whether the driver reports the
//! VID/PID/product fields reliably is a per-machine fact. What would prove it: on the live machine,
//! `board.probe`'s enumeration result shows the board (or the Arduino-name fallback fires) — see
//! `tests/live_only.rs`.

/// One enumerated port, reduced to what the choice needs: name, USB ids when the OS reports them, and
/// the strings v2's fallback matches on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortInfo {
    /// e.g. `COM5` — the name to open.
    pub name: String,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    /// The device product name the OS reports (e.g. "Arduino Leonardo"); absent on many USB serial
    /// adapters and on non-USB ports.
    pub product: Option<String>,
    /// The USB manufacturer string, e.g. "Arduino LLC"; used by the fallback too.
    pub manufacturer: Option<String>,
}

impl PortInfo {
    /// A port with no USB information (a PCI/BT/unknown port).
    pub fn plain(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            vid: None,
            pid: None,
            product: None,
            manufacturer: None,
        }
    }

    /// A USB port with ids and a product name.
    pub fn usb(name: impl Into<String>, vid: u16, pid: u16, product: Option<String>) -> Self {
        Self {
            name: name.into(),
            vid: Some(vid),
            pid: Some(pid),
            product,
            manufacturer: None,
        }
    }

    #[must_use]
    pub fn with_manufacturer(mut self, manufacturer: Option<String>) -> Self {
        self.manufacturer = manufacturer;
        self
    }

    /// v2's fallback predicate: the device's name mentions Arduino (case-insensitively). The product
    /// and manufacturer strings are what the OS gives us in place of v2's WMI `Name`; a port name
    /// mentioning it is accepted too, so a renamed port cannot hide the board.
    pub fn mentions_arduino(&self) -> bool {
        fn mentions(text: &str) -> bool {
            text.to_lowercase().contains("arduino")
        }
        self.product.as_deref().is_some_and(mentions)
            || self.manufacturer.as_deref().is_some_and(mentions)
            || mentions(&self.name)
    }
}

/// Whether `port` is the configured board by USB identity.
#[must_use]
pub fn matches_ids(port: &PortInfo, vid: u16, pids: &[u16]) -> bool {
    port.vid == Some(vid) && port.pid.is_some_and(|pid| pids.contains(&pid))
}

/// v2's discovery: the first VID/PID match, else the first port whose name mentions Arduino.
#[must_use]
pub fn discover(ports: &[PortInfo], vid: u16, pids: &[u16]) -> Option<String> {
    ports
        .iter()
        .find(|port| matches_ids(port, vid, pids))
        .or_else(|| ports.iter().find(|port| port.mentions_arduino()))
        .map(|port| port.name.clone())
}

/// The port names, as [`crate::port_choice::choose`] takes them.
#[must_use]
pub fn names(ports: &[PortInfo]) -> Vec<String> {
    ports.iter().map(|port| port.name.clone()).collect()
}

/// The whole decision: the configured override, else v2's discovery, over an injected port list.
///
/// Returns the choice and the reason there isn't one, exactly as
/// [`crate::port_choice::choose`] does; "nothing found" is `(None, None)` for the caller to explain
/// (only it knows the configured VID/PIDs to name).
pub fn choose_port(
    configured: Option<&str>,
    ports: &[PortInfo],
    vid: u16,
    pids: &[u16],
) -> (Option<String>, Option<String>) {
    let names = names(ports);
    let available: Vec<&str> = names.iter().map(String::as_str).collect();
    crate::port_choice::choose(configured, &available, || discover(ports, vid, pids))
}

/// Enumerate the machine's serial ports. The only line in this module that calls the OS: the
/// driver's report is data this crate does not invent. Errors become an empty list (v2's `Find`
/// treats an enumeration failure as "nothing found"), so a machine with no ports is not a crash.
#[cfg(windows)]
pub fn available_ports() -> Vec<PortInfo> {
    match serialport::available_ports() {
        Ok(ports) => ports.into_iter().map(from_serial).collect(),
        Err(_) => Vec::new(),
    }
}

#[cfg(windows)]
fn from_serial(info: serialport::SerialPortInfo) -> PortInfo {
    match info.port_type {
        serialport::SerialPortType::UsbPort(usb) => PortInfo {
            name: info.port_name,
            vid: Some(usb.vid),
            pid: Some(usb.pid),
            product: usb.product,
            manufacturer: usb.manufacturer,
        },
        _ => PortInfo::plain(info.port_name),
    }
}

/// No serial ports to enumerate on a non-Windows build (the dependency is Windows-only).
#[cfg(not(windows))]
pub fn available_ports() -> Vec<PortInfo> {
    Vec::new()
}
