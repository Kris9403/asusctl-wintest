use std::sync::{Arc, Mutex};

use log::{debug, error, info};
use rog_aura::keyboard::LaptopAuraPower;
use rog_aura::{AuraDeviceType, PowerZones};
use rog_dbus::zbus_aura::AuraProxy;
use slint::{ComponentHandle, Model, RgbaColor, SharedString};

use crate::config::Config;
use crate::ui::show_toast;
use crate::{
    set_ui_callbacks, set_ui_props_async, AuraPageData, MainWindow, PowerZones as SlintPowerZones,
};

fn decode_hex(s: &str) -> RgbaColor<u8> {
    let s = s.trim_start_matches('#');
    if s.len() < 6 {
        return RgbaColor {
            alpha: 255,
            red: 0,
            green: 0,
            blue: 0,
        };
    }
    let c: Vec<u8> = (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap_or(164))
        .collect();
    RgbaColor {
        alpha: 255,
        red: *c.first().unwrap_or(&255),
        green: *c.get(1).unwrap_or(&128),
        blue: *c.get(2).unwrap_or(&32),
    }
}

async fn find_aura_iface() -> Result<AuraProxy<'static>, Box<dyn std::error::Error>> {
    let conn = zbus::Connection::system().await?;
    let mgr = zbus::fdo::ObjectManagerProxy::new(&conn, "xyz.ljones.Asusd", "/").await?;
    let objs = mgr.get_managed_objects().await?;
    let mut paths: Vec<zbus::zvariant::OwnedObjectPath> = objs
        .iter()
        .filter(|(_, ifaces)| ifaces.keys().any(|k| k.as_str() == "xyz.ljones.Aura"))
        .map(|(p, _)| p.clone())
        .collect();
    if paths.len() > 1 {
        log::debug!("Multiple aura devices: {paths:?}");
    }
    let path = paths.pop().ok_or("No Aura interface")?;
    AuraProxy::builder(&conn)
        .path(path)?
        .destination("xyz.ljones.Asusd")?
        .build()
        .await
        .map_err(Into::into)
}

pub async fn prefetch_supported_basic_modes() -> Option<Vec<i32>> {
    let proxy = find_aura_iface().await.ok()?;
    let modes = proxy.supported_basic_modes().await.ok()?;
    Some(modes.iter().map(|n| (*n).into()).collect())
}

pub fn setup_aura_page(
    ui: &MainWindow,
    _states: Arc<Mutex<Config>>,
    prefetched_supported: Option<Vec<i32>>,
) {
    let g = ui.global::<AuraPageData>();
    g.on_cb_hex_from_colour(|c| {
        format!("#{:02X}{:02X}{:02X}", c.red(), c.green(), c.blue()).into()
    });
    g.on_cb_hex_to_colour(|s| decode_hex(s.as_str()).into());

    let handle = ui.as_weak();
    tokio::spawn(async move {
        let Ok(aura) = find_aura_iface().await else {
            info!("No aura interfaces");
            return Ok::<(), zbus::Error>(());
        };

        set_ui_props_async!(handle, aura, AuraPageData, brightness);
        set_ui_props_async!(handle, aura, AuraPageData, led_power);
        set_ui_props_async!(handle, aura, AuraPageData, device_type);

        let modes_vec: Vec<i32> = match prefetched_supported {
            Some(p) => p,
            None => aura
                .supported_basic_modes()
                .await
                .ok()
                .map(|m| m.iter().map(|n| (*n).into()).collect())
                .unwrap_or_default(),
        };

        // Restore saved mode, colours, zone, speed, direction from asusd (persisted to disk).
        // Use effect.mode as single source — avoid led_mode() which can fail (try_lock).
        let restore = aura.led_mode_data().await.ok();
        let raw_mode: Option<i32> = restore.as_ref().map(|d| d.mode.into());
        let d_slint = restore.map(|d| d.into());
        handle
            .upgrade_in_event_loop(move |h| {
                let names = h.global::<AuraPageData>().get_mode_names();
                let mut raws = Vec::new();
                let mut mode_names = Vec::new();
                for (i, name) in names.iter().enumerate() {
                    let raw = i as i32;
                    if modes_vec.contains(&raw) && i != 9 {
                        raws.push(raw);
                        mode_names.push(name.clone());
                    }
                }
                h.global::<AuraPageData>()
                    .set_supported_basic_modes(raws.as_slice().into());
                h.global::<AuraPageData>()
                    .set_available_mode_names(mode_names.as_slice().into());
                if let Some(d) = d_slint {
                    h.global::<AuraPageData>().invoke_update_led_mode_data(d);
                    if let Some(cm) = raw_mode {
                        let idx = raws.iter().position(|&r| r == cm).unwrap_or(0) as i32;
                        h.global::<AuraPageData>().set_current_available_mode(idx);
                    }
                    h.invoke_external_colour_change();
                }
            })
            .map_err(|e| error!("{e}"))
            .ok();

        if let Ok(mut pow3r) = aura.supported_power_zones().await {
            let dev = aura
                .device_type()
                .await
                .unwrap_or(AuraDeviceType::LaptopKeyboard2021);
            handle
                .upgrade_in_event_loop(move |handle| {
                    let names: Vec<SharedString> = handle
                        .global::<AuraPageData>()
                        .get_power_zone_names()
                        .iter()
                        .collect();
                    if dev.is_old_laptop() {
                        if pow3r.contains(&PowerZones::Keyboard)
                            && pow3r.contains(&PowerZones::Lightbar)
                        {
                            pow3r.push(PowerZones::KeyboardAndLightbar);
                        }
                        let n: Vec<SharedString> =
                            pow3r.iter().map(|z| names[(*z) as usize].clone()).collect();
                        handle
                            .global::<AuraPageData>()
                            .set_power_zone_names_old(n.as_slice().into());
                    } else {
                        let p: Vec<SlintPowerZones> = pow3r.iter().map(|z| (*z).into()).collect();
                        handle
                            .global::<AuraPageData>()
                            .set_supported_power_zones(p.as_slice().into());
                    }
                })
                .ok();
        }

        let proxy = aura.clone();
        let weak = handle.clone();
        handle
            .upgrade_in_event_loop(move |h| {
                set_ui_callbacks!(h,
                    AuraPageData(.into()),
                    proxy.brightness(.into()),
                    "Brightness set to {}",
                    "Brightness failed"
                );

                let p = proxy.clone();
                let w = weak.clone();
                h.global::<AuraPageData>().on_apply_led_mode_data(move || {
                    let Some(ui) = w.upgrade() else { return };
                    let slint_effect = ui.global::<AuraPageData>().get_led_mode_data();
                    let raw: rog_aura::AuraEffect = slint_effect.into();
                    let pp = p.clone();
                    let t = w.clone();
                    tokio::spawn(async move {
                        let r = pp.set_led_mode_data(raw).await;
                        show_toast("LED mode applied".into(), "LED mode failed".into(), t, r);
                    });
                });
                h.invoke_external_colour_change();
            })
            .ok();

        // G615LR-specific, experimental: fires a hardcoded test pattern via
        // the isolated WriteLightbar2025Zones D-Bus method. Deliberately
        // separate from the on_apply_led_mode_data wiring above -- doesn't
        // touch AuraEffect/led_mode_data at all. Not upstreamed; local-only
        // while the underlying 0x04 protocol is still under investigation,
        // see HANDOFF.md in the asusctl-wintest research repo. Test pattern
        // below is the exact 12 zone/colour pairs already human-confirmed
        // correct on real hardware (Windows session 3,
        // usb_capture_session4/multizone_12x_confirmed.pcapng), so a
        // successful run here has a known-correct result to check against.
        let p_lightbar = aura.clone();
        let w_lightbar = handle.clone();
        handle
            .upgrade_in_event_loop(move |h| {
                h.global::<AuraPageData>()
                    .on_cb_lightbar_2025_test(move || {
                        let Some(_ui) = w_lightbar.upgrade() else {
                            return;
                        };
                        let p = p_lightbar.clone();
                        let t = w_lightbar.clone();
                        tokio::spawn(async move {
                            // (wire_zone_id, r, g, b) -- see
                            // rog_aura::lightbar_2025::Lightbar2025Zone for
                            // what each ID physically is.
                            let zones: Vec<(u8, u8, u8, u8)> = vec![
                                (0x00, 0xFF, 0x00, 0x00), // Keyboard1
                                (0x01, 0x00, 0xFF, 0x00), // Keyboard2
                                (0x02, 0x00, 0x00, 0xFF), // Keyboard3
                                (0x03, 0xFF, 0xFF, 0xFF), // Keyboard4
                                (0x05, 0xFF, 0x00, 0x00), // BackBarLeft
                                (0x04, 0x00, 0xFF, 0x00), // BackBarRight
                                (0x07, 0x00, 0x00, 0xFF), // CornerBackLeft
                                (0x06, 0xFF, 0xFF, 0x00), // CornerBackRight
                                (0x0C, 0xFF, 0x80, 0x00), // CornerFrontRight
                                (0x0D, 0xFF, 0xFF, 0xFF), // CornerFrontLeft
                                (0x0E, 0x00, 0xFF, 0xFF), // FrontBarRight
                                (0x0F, 0xFF, 0x00, 0xFF), // FrontBarLeft
                            ];
                            let r = p.write_lightbar_2025_zones(zones).await;
                            show_toast(
                                "Lightbar 2025 test pattern sent".into(),
                                "Lightbar 2025 test failed".into(),
                                t,
                                r,
                            );
                        });
                    });
            })
            .map_err(|e| error!("{e}"))
            .ok();

        let weak_power = handle.clone();
        let proxy_power = aura.clone();
        handle
            .upgrade_in_event_loop(|h| {
                h.global::<AuraPageData>().on_cb_led_power(move |power| {
                    let w = weak_power.clone();
                    let p = proxy_power.clone();
                    let pw: LaptopAuraPower = power.into();
                    tokio::spawn(async move {
                        show_toast(
                            "Aura power updated".into(),
                            "Aura power failed".into(),
                            w,
                            p.set_led_power(pw).await,
                        );
                    });
                });
            })
            .map_err(|e| error!("{e}"))
            .ok();

        let stream_handle = handle.clone();
        let aura_stream = aura.clone();
        tokio::spawn(async move {
            use futures_util::StreamExt;
            let mut stream = aura_stream.receive_led_mode_data_changed().await;
            while let Some(e) = stream.next().await {
                if let Ok(out) = e.get().await {
                    let raw: i32 = out.mode.into();
                    let data = out.into();
                    stream_handle
                        .upgrade_in_event_loop(move |h| {
                            h.global::<AuraPageData>().invoke_update_led_mode_data(data);
                            let supported: Vec<i32> = h
                                .global::<AuraPageData>()
                                .get_supported_basic_modes()
                                .iter()
                                .collect();
                            let idx = supported.iter().position(|&x| x == raw).unwrap_or(0) as i32;
                            h.global::<AuraPageData>().set_current_available_mode(idx);
                            h.invoke_external_colour_change();
                        })
                        .map_err(|e| error!("{e}"))
                        .ok();
                }
            }
        });
        debug!("Aura setup done");
        Ok(())
    });
}
