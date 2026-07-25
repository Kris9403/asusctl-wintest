use std::error::Error;
use std::time::Duration;

/// MAJOR NEW LEAD, found by digging into this repo's own EXISTING code
/// rather than guessing new bytes: `rog-aura/src/keyboard/advanced.rs`
/// already implements a COMPLETE, real, third protocol -- `0x5d` with
/// mode byte `0xbc` ("custom"/direct-addressing mode, distinct from the
/// `0xb3` "builtin" mode used everywhere else this session) -- for
/// laptops with `advanced_type: PerKey` in `aura_support.ron`.
///
/// `LedUsbPackets::get_init_msg()`'s own doc comment: "Initialise and
/// clear the keyboard for custom effects, this must be done every time
/// mode switches from builtin to custom." We have NEVER sent this
/// (`5d bc 00...`) this entire investigation -- every prior "priming"
/// attempt used `5d b3` (builtin mode), never `5d bc` (custom mode).
///
/// Critically: G615LR's OWN `aura_support.ron` entry has `layout_name:
/// "g634j-per-key"` -- it already explicitly references the G634J
/// per-key layout. G634J and G635L (the closest sibling models, same
/// generation, same basic_modes/power_zones) both use `advanced_type:
/// PerKey`, meaning THEY already get real per-key/zoned direct
/// addressing through this exact protocol. G615LR's `advanced_type` was
/// simply left as `r#None` -- nobody ever tried routing it through this
/// already-working mechanism. All prior investigation this session
/// focused entirely on the separate `0x04` protocol and never circled
/// back to this one.
///
/// `LedUsbPackets::new_zoned(true)` builds: `5d bc 01 01 04 [zeros]`,
/// then RGB triples at fixed offsets for `ZonedKbLeft`(9),
/// `ZonedKbLeftMid`(12), `ZonedKbRightMid`(15), `ZonedKbRight`(18),
/// `LightbarRight`(27), `LightbarRightCorner`(30),
/// `LightbarRightBottom`(33), `LightbarLeftBottom`(36),
/// `LightbarLeftCorner`(39), `LightbarLeft`(42) -- a real, already-coded
/// zoned lightbar protocol distinct from both `0x5d b3` (whole-chassis)
/// and `0x04` (16-zone).
///
/// Test: send the real init message, then a zoned packet with every
/// keyboard zone and every lightbar code set to a distinct bright
/// colour simultaneously, via the same Output-report control-transfer
/// mechanism (`wValue=0x025d`) `write_bytes`/hidraw's plain write()
/// resolves to for this report ID (confirmed via the report descriptor
/// dump earlier this session: 0x5d has real Output report capability).
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

    // Dark reset first (real Static black, builtin mode, zone=None) so
    // we start from a confirmed baseline, same as every other test.
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

    // THE REAL LEAD: get_init_msg() -- mode 0xbc, "custom" mode, never
    // sent this whole investigation.
    let mut init_msg = [0u8; 64];
    init_msg[0] = 0x5d;
    init_msg[1] = 0xbc;
    send!("0x025d bc (CUSTOM MODE INIT -- never tried before) iface0", 0x09, 0x025du16, 0u16, &init_msg);
    println!("Custom-mode init sent. Waiting 1s...");
    std::thread::sleep(Duration::from_secs(1));

    // new_zoned(true): 5d bc 01 01 04, then RGB triples at fixed offsets.
    let mut zoned = [0u8; 64];
    zoned[0] = 0x5d;
    zoned[1] = 0xbc;
    zoned[2] = 0x01;
    zoned[3] = 0x01;
    zoned[4] = 0x04; // multizoned flag

    // Keyboard zones: distinct colours per zone.
    zoned[9] = 0xff; zoned[10] = 0x00; zoned[11] = 0x00;  // ZonedKbLeft: red
    zoned[12] = 0x00; zoned[13] = 0xff; zoned[14] = 0x00; // ZonedKbLeftMid: green
    zoned[15] = 0x00; zoned[16] = 0x00; zoned[17] = 0xff; // ZonedKbRightMid: blue
    zoned[18] = 0xff; zoned[19] = 0xff; zoned[20] = 0x00; // ZonedKbRight: yellow

    // Lightbar codes: distinct colours per code.
    zoned[27] = 0xff; zoned[28] = 0x00; zoned[29] = 0xff; // LightbarRight: magenta
    zoned[30] = 0x00; zoned[31] = 0xff; zoned[32] = 0xff; // LightbarRightCorner: cyan
    zoned[33] = 0xff; zoned[34] = 0x80; zoned[35] = 0x00; // LightbarRightBottom: orange
    zoned[36] = 0x80; zoned[37] = 0x00; zoned[38] = 0xff; // LightbarLeftBottom: purple
    zoned[39] = 0xff; zoned[40] = 0xff; zoned[41] = 0xff; // LightbarLeftCorner: white
    zoned[42] = 0xff; zoned[43] = 0xff; zoned[44] = 0x00; // LightbarLeft: yellow

    send!("0x025d bc (ZONED colour packet, ALL zones+lightbar codes set) iface0", 0x09, 0x025du16, 0u16, &zoned);

    println!("Sent. Watch EVERYTHING -- keyboard zones (red/green/blue/yellow left-to-right) AND all 6 lightbar codes (magenta/cyan/orange/purple/white/yellow). Holding for 10s...");
    std::thread::sleep(Duration::from_secs(10));

    println!("Done.");
    Ok(())
}
