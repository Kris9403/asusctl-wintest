use std::error::Error;
use std::time::Duration;

/// Fetches the FULL Extended Properties OS Feature Descriptor (76 bytes,
/// confirmed present via the header-only probe in
/// `g615lr-msos-compatid.rs`) for both interfaces. This is a real,
/// Windows-only enumeration-time mechanism that can carry arbitrary
/// custom registry properties ASUS's driver installation applies
/// automatically -- something Linux's kernel has no knowledge of or
/// interaction with whatsoever. Pure read-only GET_DESCRIPTOR-style
/// vendor request, safe.
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

    for iface in [0u16, 1u16] {
        println!("=== Interface {iface} ===");
        let mut buf = vec![0u8; 76];
        let r = handle.read_control(0xC1, VENDOR_CODE, iface, 0x0005, &mut buf, Duration::from_secs(2));
        match r {
            Ok(n) => {
                println!("Got {n} bytes: {:02x?}", &buf[..n]);
                if n >= 10 {
                    let total_len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
                    let version = u16::from_le_bytes([buf[4], buf[5]]);
                    let w_index = u16::from_le_bytes([buf[6], buf[7]]);
                    let count = u16::from_le_bytes([buf[8], buf[9]]);
                    println!("  dwLength={total_len} bcdVersion={version:#06x} wIndex={w_index:#06x} wCount={count}");

                    // Parse property sections starting at offset 10.
                    let mut off = 10usize;
                    let mut idx = 1;
                    while off + 4 <= n {
                        let prop_len = u32::from_le_bytes([buf[off], buf[off+1], buf[off+2], buf[off+3]]) as usize;
                        if prop_len == 0 || off + prop_len > n {
                            break;
                        }
                        let data_type = u32::from_le_bytes([buf[off+4], buf[off+5], buf[off+6], buf[off+7]]);
                        let name_len = u16::from_le_bytes([buf[off+8], buf[off+9]]) as usize;
                        let name_start = off + 10;
                        let name_end = (name_start + name_len).min(n);
                        let name_utf16: Vec<u16> = buf[name_start..name_end]
                            .chunks(2)
                            .filter(|c| c.len() == 2)
                            .map(|c| u16::from_le_bytes([c[0], c[1]]))
                            .collect();
                        let name = String::from_utf16_lossy(&name_utf16);

                        let data_len_start = name_end;
                        if data_len_start + 4 <= n {
                            let data_len = u32::from_le_bytes([
                                buf[data_len_start], buf[data_len_start+1],
                                buf[data_len_start+2], buf[data_len_start+3],
                            ]) as usize;
                            let data_start = data_len_start + 4;
                            let data_end = (data_start + data_len).min(n);
                            let data_bytes = &buf[data_start..data_end];
                            let data_utf16: Vec<u16> = data_bytes
                                .chunks(2)
                                .filter(|c| c.len() == 2)
                                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                                .collect();
                            let data_str = String::from_utf16_lossy(&data_utf16);

                            println!("  Property {idx}: dwPropertyDataType={data_type:#x} name={name:?} data={data_str:?} raw_data={data_bytes:02x?}");
                        }
                        off += prop_len;
                        idx += 1;
                    }
                }
            }
            Err(e) => println!("Failed: {e:?}"),
        }
        println!();
    }

    Ok(())
}
