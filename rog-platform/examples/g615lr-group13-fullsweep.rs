use std::error::Error;
use std::time::Duration;

/// Focused, single-group full sweep: group 13 only (the one recalled
/// working during the earlier 3s-hold test), every offset from 1 through
/// 61 (the maximum possible 3-byte-aligned start in a 64-byte packet --
/// byte 63 is the last valid index, so 61/62/63 is the final triple that
/// fits), 1.5s hold each for easier visual recognition than the earlier
/// 0.5s sweep.
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

    let had0 = handle.kernel_driver_active(0).unwrap_or(false);
    if had0 {
        handle.detach_kernel_driver(0)?;
    }
    let had1 = handle.kernel_driver_active(1).unwrap_or(false);
    if had1 {
        handle.detach_kernel_driver(1)?;
    }
    handle.claim_interface(0)?;
    handle.claim_interface(1)?;

    struct RestoreGuard<'a, T: rusb::UsbContext> {
        handle: &'a rusb::DeviceHandle<T>,
        had0: bool,
        had1: bool,
    }
    impl<'a, T: rusb::UsbContext> Drop for RestoreGuard<'a, T> {
        fn drop(&mut self) {
            let _ = self.handle.release_interface(0);
            let _ = self.handle.release_interface(1);
            if self.had0 {
                let _ = self.handle.attach_kernel_driver(0);
            }
            if self.had1 {
                let _ = self.handle.attach_kernel_driver(1);
            }
        }
    }
    let _restore_guard = RestoreGuard { handle: &handle, had0, had1 };

    macro_rules! send {
        ($label:expr, $req:expr, $val:expr, $idx:expr, $data:expr) => {
            let r = handle.write_control(0x21, $req, $val, $idx, $data, Duration::from_secs(2));
            println!("{}: {:?}", $label, r);
        };
    }

    send!("SET_IDLE iface1", 0x0a, 0x0000u16, 1u16, &[]);
    send!("SET_IDLE iface0", 0x0a, 0x0000u16, 0u16, &[]);
    send!("0x0201 (01 01) iface0", 0x09, 0x0201u16, 0u16, &[0x01, 0x01]);

    #[rustfmt::skip]
    let dark_b3: [u8; 64] = [
        0x5d, 0xb3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];
    let mut dark_b5 = [0u8; 64];
    dark_b5[0] = 0x5d;
    dark_b5[1] = 0xb5;
    let mut dark_b4 = [0u8; 64];
    dark_b4[0] = 0x5d;
    dark_b4[1] = 0xb4;
    send!("0x025d b3 (dark reset) iface0", 0x09, 0x025du16, 0u16, &dark_b3);
    send!("0x025d b5 (set) iface0", 0x09, 0x025du16, 0u16, &dark_b5);
    send!("0x025d b4 (apply) iface0", 0x09, 0x025du16, 0u16, &dark_b4);
    println!("Dark reset sent. Waiting 2s...");
    std::thread::sleep(Duration::from_secs(2));

    let mut init_msg = [0u8; 64];
    init_msg[0] = 0x5d;
    init_msg[1] = 0xbc;
    send!("0x025d bc (CUSTOM MODE INIT) iface0", 0x09, 0x025du16, 0u16, &init_msg);
    println!("Custom-mode init sent. Waiting 1s...");
    std::thread::sleep(Duration::from_secs(1));

    const GROUP: u8 = 13;

    fn test_offset(handle: &rusb::DeviceHandle<impl rusb::UsbContext>, offset: usize) {
        let mut pkt = [0u8; 64];
        pkt[0] = 0x5d;
        pkt[1] = 0xbc;
        pkt[2] = 0x00;
        pkt[3] = 0x01;
        pkt[4] = 0x01;
        pkt[5] = 0x01;
        pkt[6] = GROUP << 4;
        pkt[7] = 0x10;
        pkt[8] = 0x00;
        pkt[offset] = 0xff;
        pkt[offset + 1] = 0xff;
        pkt[offset + 2] = 0xff;
        println!("--- group 13 offset {offset}: writing white ---");
        let r = handle.write_control(0x21, 0x09, 0x025du16, 0u16, &pkt, Duration::from_secs(1));
        if r.is_err() {
            println!("  write failed: {r:?}");
        }
        std::thread::sleep(Duration::from_millis(1500));

        let mut dark = [0u8; 64];
        dark[0] = 0x5d;
        dark[1] = 0xbc;
        dark[2] = 0x00;
        dark[3] = 0x01;
        dark[4] = 0x01;
        dark[5] = 0x01;
        dark[6] = GROUP << 4;
        dark[7] = 0x10;
        let _ = handle.write_control(0x21, 0x09, 0x025du16, 0u16, &dark, Duration::from_secs(1));
        std::thread::sleep(Duration::from_millis(300));
    }

    println!("Sweeping group 13, offsets 1-61, 1.5s hold each...");
    for offset in 1usize..=61 {
        test_offset(&handle, offset);
    }

    println!("Sweep complete. Note which offset(s) (if any) showed ANYTHING at all.");
    Ok(())
}
