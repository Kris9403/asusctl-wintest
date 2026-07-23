use std::cell::RefCell;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::PathBuf;

use log::{info, warn};
use udev::Device;

use crate::error::{PlatformError, Result};

// HIDIOCSFEATURE(len) from <linux/hidraw.h>: `_IOC(_IOC_WRITE|_IOC_READ, 'H',
// 0x06, len)`. This is what an HID **Feature** report requires on Linux --
// the plain `write_bytes()` below sends an **Output** report instead, which
// is a different report type some devices (e.g. the G615LR's undocumented
// per-zone lightbar protocol, report ID 0x04) silently ignore. nix's
// `ioctl_readwrite_buf!` recomputes the request code from the buffer's
// actual length at call time, matching the C macro's per-length behavior.
nix::ioctl_readwrite_buf!(hidiocsfeature, b'H', 0x06, u8);

/// A USB device that utilizes hidraw for I/O
#[derive(Debug)]
pub struct HidRaw {
    /// The path to the `/dev/<name>` of the device
    devfs_path: PathBuf,
    /// The sysfs path
    syspath: PathBuf,
    /// The product ID. The vendor ID is not kept
    prod_id: String,
    _device_bcd: u32,
    /// Retaining a handle to the file for the duration of `HidRaw`
    file: RefCell<File>,
}

impl HidRaw {
    pub fn new(id_product: &str) -> Result<Self> {
        let mut enumerator = udev::Enumerator::new().map_err(|err| {
            warn!("{}", err);
            PlatformError::Udev("enumerator failed".into(), err)
        })?;

        enumerator.match_subsystem("hidraw").map_err(|err| {
            warn!("{}", err);
            PlatformError::Udev("match_subsystem failed".into(), err)
        })?;

        for endpoint in enumerator
            .scan_devices()
            .map_err(|e| PlatformError::IoPath("enumerator".to_owned(), e))?
        {
            if let Some(usb_device) = endpoint
                .parent_with_subsystem_devtype("usb", "usb_device")
                .map_err(|e| {
                    PlatformError::IoPath(endpoint.devpath().to_string_lossy().to_string(), e)
                })?
            {
                if let Some(dev_node) = endpoint.devnode() {
                    if let Some(this_id_product) = usb_device.attribute_value("idProduct") {
                        if this_id_product != id_product {
                            continue;
                        }
                        let dev_path = endpoint.devpath().to_string_lossy();
                        if dev_path.contains("virtual") {
                            info!(
                                "Using device at: {:?} for <TODO: label control> control",
                                dev_node
                            );
                        }
                        return Ok(Self {
                            // read(true) is needed alongside write(true) for
                            // HIDIOCSFEATURE (see set_feature_report below) --
                            // plain Output-report write_bytes() doesn't need
                            // it, but sharing one handle for both is simpler
                            // than tracking two.
                            file: RefCell::new(
                                OpenOptions::new().read(true).write(true).open(dev_node)?,
                            ),
                            devfs_path: dev_node.to_owned(),
                            prod_id: this_id_product.to_string_lossy().into(),
                            syspath: endpoint.syspath().into(),
                            _device_bcd: usb_device
                                .attribute_value("bcdDevice")
                                .unwrap_or_default()
                                .to_string_lossy()
                                .parse()
                                .unwrap_or_default(),
                        });
                    }
                }
            }
        }
        Err(PlatformError::MissingFunction(format!(
            "hidraw dev {} not found",
            id_product
        )))
    }

    /// Make `HidRaw` device from a udev device
    pub fn from_device(endpoint: Device) -> Result<Self> {
        if let Some(parent) = endpoint
            .parent_with_subsystem_devtype("usb", "usb_device")
            .map_err(|e| {
                PlatformError::IoPath(endpoint.devpath().to_string_lossy().to_string(), e)
            })?
        {
            if let Some(dev_node) = endpoint.devnode() {
                if let Some(id_product) = parent.attribute_value("idProduct") {
                    return Ok(Self {
                        file: RefCell::new(OpenOptions::new().write(true).open(dev_node)?),
                        devfs_path: dev_node.to_owned(),
                        prod_id: id_product.to_string_lossy().into(),
                        syspath: endpoint.syspath().into(),
                        _device_bcd: endpoint
                            .attribute_value("bcdDevice")
                            .unwrap_or_default()
                            .to_string_lossy()
                            .parse()
                            .unwrap_or_default(),
                    });
                }
            }
        }
        Err(PlatformError::MissingFunction(
            "hidraw dev no dev path".to_string(),
        ))
    }

    pub fn prod_id(&self) -> &str {
        &self.prod_id
    }

    /// Write an array of raw bytes to the device using the hidraw interface
    pub fn write_bytes(&self, message: &[u8]) -> Result<()> {
        if let Ok(mut file) = self.file.try_borrow_mut() {
            // TODO: re-get the file if error?
            file.write_all(message).map_err(|e| {
                PlatformError::IoPath(self.devfs_path.to_string_lossy().to_string(), e)
            })?;
        }
        Ok(())
    }

    /// Send an HID **Feature** report via the `HIDIOCSFEATURE` ioctl, as
    /// opposed to `write_bytes()` which sends an **Output** report. Some
    /// devices only respond to one or the other for a given report ID --
    /// e.g. the G615LR's per-zone lightbar protocol (report ID `0x04`,
    /// see `rog_aura::lightbar_2025`) is Feature-report-only; sending it as
    /// an Output report via plain `write()` is accepted by the kernel but
    /// produces no visible effect on the hardware.
    ///
    /// `report` must start with the report ID byte, matching the same
    /// layout used for the Windows `HidD_SetFeature` call this mirrors.
    pub fn set_feature_report(&self, report: &[u8]) -> Result<()> {
        let mut buf = report.to_vec();
        let file = self.file.borrow();
        let n = unsafe { hidiocsfeature(file.as_raw_fd(), &mut buf) }.map_err(|errno| {
            PlatformError::IoPath(
                self.devfs_path.to_string_lossy().to_string(),
                std::io::Error::from(errno),
            )
        })?;
        info!(
            "set_feature_report on {}: sent {} bytes, kernel reported {} bytes transferred",
            self.devfs_path.to_string_lossy(),
            buf.len(),
            n
        );
        Ok(())
    }

    /// Open a specific hidraw device node directly, bypassing
    /// `idProduct`-based enumeration. `HidRaw::new` returns the *first*
    /// hidraw node matching `id_product`, which is ambiguous for devices
    /// exposing multiple HID interfaces under one `idProduct` (e.g. the
    /// G615LR: `/dev/hidraw1` is `bInterfaceNumber 00`, `/dev/hidraw2` is
    /// `01` -- only one of them carries a given report ID). Use this once
    /// you've identified the right node yourself, e.g. via `udevadm info -a
    /// -n /dev/hidrawN | grep bInterfaceNumber`.
    pub fn from_devnode(devnode: &std::path::Path, id_product: &str) -> Result<Self> {
        Ok(Self {
            file: RefCell::new(OpenOptions::new().read(true).write(true).open(devnode)?),
            devfs_path: devnode.to_owned(),
            prod_id: id_product.to_owned(),
            syspath: PathBuf::new(),
            _device_bcd: 0,
        })
    }

    /// This method was added for certain devices like AniMe to prevent them
    /// waking the laptop
    pub fn set_wakeup_disabled(&self) -> Result<()> {
        let mut dev = Device::from_syspath(&self.syspath)?;
        Ok(dev.set_attribute_value("power/wakeup", "disabled")?)
    }
}
