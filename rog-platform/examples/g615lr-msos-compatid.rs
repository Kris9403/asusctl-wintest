use std::error::Error;
use std::time::Duration;

/// Follow-up to `g615lr-msos-descriptor-check.rs`, which confirmed this
/// device implements a real Microsoft OS 1.0 descriptor ("MSFT100"
/// signature, vendor code 0x7F). This queries the Extended Compat ID
/// Descriptor using that vendor code -- reveals what Windows actually
/// does with this device during enumeration, most commonly used to tell
/// Windows to bind a specific driver (e.g. WinUSB) to a specific
/// interface, which -- if true here -- would mean Windows isn't even
/// using the same HID transport Linux is for this device's vendor
/// interface, a genuinely deep explanation for a persistent platform
/// difference no amount of runtime byte-content tweaking could fix.
///
/// Pure GET_DESCRIPTOR-style vendor request, read-only, safe. Per the MS
/// OS descriptor spec: bmRequestType=0xC0 (device-to-host, vendor,
/// device), bRequest=<vendor code>, wValue=0x0000, wIndex=0x0004
/// (Extended Compat ID), wLength=<enough for the header + N sections>.
fn main() -> Result<(), Box<dyn Error>> {
    let devices = rusb::devices()?;
    let mut target = None;
    for device in devices.iter() {
        let desc = device.device_descriptor()?;
        if desc.vendor_id() == 0x0B05 && desc.product_id() == 0x19B6 {
            target = Some(device);
            break;
        }
    }
    let device = target.ok_or("device 0B05:19B6 not found")?;
    let handle = device.open()?;

    const VENDOR_CODE: u8 = 0x7F;

    // Try Extended Properties (index 0x0005, interface 0 and 1) too, in
    // case Compat ID (0x0004) just isn't implemented but Properties is.
    for iface in [0u16, 1u16] {
        let mut buf = [0u8; 10];
        let r = handle.read_control(0xC1, VENDOR_CODE, iface, 0x0005, &mut buf, Duration::from_secs(2));
        println!("Extended Properties (interface {iface}): {r:?} {:02x?}", &buf[..r.unwrap_or(0)]);
    }

    // First read just the 10-byte header to get the real total length.
    let mut header = [0u8; 10];
    let r = handle.read_control(0xC0, VENDOR_CODE, 0x0000, 0x0004, &mut header, Duration::from_secs(2));
    println!("Header read: {r:?}");
    match r {
        Ok(n) => {
            println!("Header bytes ({n}): {:02x?}", &header[..n]);
            if n >= 8 {
                let total_len = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
                let version = u16::from_le_bytes([header[4], header[5]]);
                let w_index_echo = u16::from_le_bytes([header[6], header[7]]);
                let count = header.get(8).copied().unwrap_or(0);
                println!("  dwLength={total_len} bcdVersion={version:#06x} wIndex={w_index_echo:#06x} bCount={count}");

                if total_len > 0 && total_len < 4096 {
                    let mut full = vec![0u8; total_len as usize];
                    let r2 = handle.read_control(0xC0, VENDOR_CODE, 0x0000, 0x0004, &mut full, Duration::from_secs(2));
                    println!("Full read: {r2:?}");
                    if let Ok(n2) = r2 {
                        println!("Full descriptor ({n2} bytes): {:02x?}", &full[..n2]);
                        // Try to decode function sections (each 24 bytes starting at offset 16)
                        let mut off = 16usize;
                        let mut idx = 1;
                        while off + 24 <= full.len() {
                            let iface_num = full[off];
                            let compat_id: String = full[off + 4..off + 12]
                                .iter()
                                .filter(|&&b| b != 0)
                                .map(|&b| b as char)
                                .collect();
                            let sub_id: String = full[off + 12..off + 20]
                                .iter()
                                .filter(|&&b| b != 0)
                                .map(|&b| b as char)
                                .collect();
                            println!("  Function {idx}: interface={iface_num} compatID={compat_id:?} subCompatID={sub_id:?}");
                            off += 24;
                            idx += 1;
                        }
                    }
                }
            }
        }
        Err(e) => println!("Failed: {e:?}"),
    }

    Ok(())
}
