use std::error::Error;
use std::time::Duration;

/// New hypothesis, from a live-hardware discovery this session (not a
/// guess): with the classic `0x5d` protocol, once ANY animated mode is
/// actively running (triggered globally, zone=None), NEW zone-scoped
/// writes on top of it get ABSORBED into that already-running animation
/// loop rather than creating independent state -- confirmed live: with
/// `breathe --zone 1` (blue) already running, sending `static --zone 2
/// -c ffff00` (a STATIC command, not breathe) made zone 2 start
/// BREATHING green, synced with zone 1. The requested mode only matters
/// for the write that first STARTS the loop; every write after that just
/// updates a zone's colour inside the running animation.
///
/// This matches something we already found for `0x04`: a 40s continuous
/// `0x04` stream against an active `0x5d` RainbowCycle produced a subtle
/// flicker synced to every write (`g615lr-literal-30s-stream.rs`) -- the
/// writes DO land, but get overwritten on the animation's very next tick.
/// BUT that was only ever tested against RainbowCycle, which is PURELY
/// PROCEDURAL -- it has no colour parameter to read at all, so there's
/// nothing for it to "absorb" from a new write. Breathe DOES have a real
/// colour parameter it clearly re-reads every tick (per the zone-2
/// discovery above). This test asks: does an active Breathe loop (not
/// RainbowCycle) actually pick up and persistently render `0x04` zone
/// writes, the same way it just picked up a classic-protocol zone-2
/// write?
///
/// Sequence: dark reset -> trigger global Breathe (zone=None, mode=0x01,
/// red) via the real b3,b5,b4 order -> wait for it to visibly be
/// breathing -> stream 0x04 zone writes (a real single zone, distinct
/// colour) for 15s, watching for the zone to persistently show ITS OWN
/// colour (not just flicker, not just breathing in the global colour).
/// Reattaches the kernel HID driver to both interfaces on drop -- including
/// during a panic unwind. Without this, a bug anywhere below (like the
/// hex-length panic that bit us once already) leaves interface 0 detached
/// from `usbhid`, which is the SAME interface the physical keyboard's boot
/// input lives on -- i.e. it silently kills the built-in keyboard until
/// someone manually rebinds it via
/// `echo -n "5-4:1.0" | sudo tee /sys/bus/usb/drivers/usbhid/bind`.
/// Restarting asusd does NOT fix this -- kernel driver binding is
/// independent of any userspace daemon.
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
    // From here on, ANY early return or panic still restores the kernel
    // driver -- this guard's Drop runs regardless.
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

    // Dark reset first (real Static black, zone=None, b3,b5,b4 order).
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
    println!("Dark reset sent. Waiting 2s to confirm dark...");
    std::thread::sleep(Duration::from_secs(2));

    // Global Breathe (zone=None=0x00, mode=Breathe=0x01, red), real
    // b3,b5,b4 order, same speed byte (0xeb) seen in every other
    // proven-correct capture this session.
    #[rustfmt::skip]
    let breathe_b3: [u8; 64] = [
        0x5d, 0xb3, 0x00, 0x01, 0xff, 0x00, 0x00, 0xeb, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];
    let mut breathe_b5 = [0u8; 64];
    breathe_b5[0] = 0x5d;
    breathe_b5[1] = 0xb5;
    let mut breathe_b4 = [0u8; 64];
    breathe_b4[0] = 0x5d;
    breathe_b4[1] = 0xb4;
    send!("0x025d b3 (global Breathe, red) iface0", 0x09, 0x025du16, 0u16, &breathe_b3);
    send!("0x025d b5 (set) iface0", 0x09, 0x025du16, 0u16, &breathe_b5);
    send!("0x025d b4 (apply) iface0", 0x09, 0x025du16, 0u16, &breathe_b4);

    println!("Global Breathe activated -- should be visibly breathing red now. Waiting 3s to confirm it's actually animating...");
    std::thread::sleep(Duration::from_secs(3));

    let handshake05: [u8; 10] = [0x05, 0x00, 0x08, 0x00, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x01];
    send!("0x0305 (handshake) iface1", 0x09, 0x0305u16, 1u16, &handshake05);

    // A single, distinct zone -- kbd3 (0x02), the "cleanly isolated, no
    // lightbar bleed" zone per this session's classic-protocol findings,
    // set to bright green. Literal bytes, same 51-byte packet format
    // proven correct all session.
    let zone_hex = "0401010200000000000000000000000000000000ff00ff00000000000000000000000000000000000000000000000000000000";
    let zone_bytes: Vec<u8> = (0..zone_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&zone_hex[i..i + 2], 16).unwrap())
        .collect();
    assert_eq!(zone_bytes.len(), 51);

    println!("Streaming 0x04 zone 0x02 (green) for 15 seconds over the active global Breathe (red)...");
    let start = std::time::Instant::now();
    let mut cycles = 0u32;
    while start.elapsed() < Duration::from_secs(15) {
        let r = handle.write_control(0x21, 0x09, 0x0304u16, 1u16, &zone_bytes, Duration::from_millis(500));
        if r.is_err() {
            println!("  write: {r:?}");
        }
        cycles += 1;
        std::thread::sleep(Duration::from_millis(200));
    }
    println!("Done streaming: {cycles} writes over 15s.");
    // Interface release + kernel driver reattach now happens automatically
    // via RestoreGuard's Drop impl, even on panic.

    println!("Done. Watch for: does zone 0x02 (kbd3) show a PERSISTENT green (breathing or not), distinct from the rest of the chassis breathing red? Or just a flicker like the RainbowCycle test showed?");
    Ok(())
}
