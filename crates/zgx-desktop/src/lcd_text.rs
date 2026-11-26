//! Whether a window's text is drawn for an LCD screen's subpixels, and in
//! which order (`GpuRenderer::set_lcd_text`): on Windows as ClearType is set
//! (font smoothing on, of ClearType's type, in its subpixel orientation),
//! read when first asked and again on each settings change
//! (`window_chrome`'s `sync`); elsewhere plain coverage. `ZGX_LCD_TEXT`
//! overrides it: `0` or `off` never, `rgb` or `bgr` always.
use std::sync::atomic::{AtomicU8, Ordering};
use zgx_gpu::LcdOrder;

/// As last read: 0 unread, else `encode`'s.
static ORDER: AtomicU8 = AtomicU8::new(0);

/// The order a window's text is drawn for now.
pub(crate) fn order() -> Option<LcdOrder> {
    match ORDER.load(Ordering::Relaxed) {
        0 => decode(read()),
        value => decode(value),
    }
}

/// Read the setting again (a settings change: ClearType turned on or off,
/// or tuned for another screen).
pub(crate) fn sync() {
    read();
}

fn read() -> u8 {
    let order = std::env::var("ZGX_LCD_TEXT")
        .ok()
        .and_then(|value| parse(&value))
        .unwrap_or_else(system);
    let value = encode(order);
    ORDER.store(value, Ordering::Relaxed);
    value
}

/// `ZGX_LCD_TEXT`'s value: `Some(None)` off, `Some(Some(order))` on in
/// that order, None not understood (the system's setting then).
fn parse(value: &str) -> Option<Option<LcdOrder>> {
    match value.trim().to_ascii_lowercase().as_str() {
        "0" | "off" | "false" | "none" => Some(None),
        "1" | "on" | "rgb" => Some(Some(LcdOrder::Rgb)),
        "bgr" => Some(Some(LcdOrder::Bgr)),
        _ => None,
    }
}

fn encode(order: Option<LcdOrder>) -> u8 {
    match order {
        None => 1,
        Some(LcdOrder::Rgb) => 2,
        Some(LcdOrder::Bgr) => 3,
    }
}

fn decode(value: u8) -> Option<LcdOrder> {
    match value {
        2 => Some(LcdOrder::Rgb),
        3 => Some(LcdOrder::Bgr),
        _ => None,
    }
}

/// ClearType as Windows has it (Settings, "Adjust ClearType text"): on when
/// font smoothing is on and of ClearType's type, in its orientation.
#[cfg(windows)]
fn system() -> Option<LcdOrder> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_GETFONTSMOOTHING, SPI_GETFONTSMOOTHINGORIENTATION, SPI_GETFONTSMOOTHINGTYPE,
        SYSTEM_PARAMETERS_INFO_ACTION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    };
    // `FE_FONTSMOOTHINGCLEARTYPE` and `FE_FONTSMOOTHINGORIENTATIONBGR`.
    const CLEARTYPE: u32 = 2;
    const BGR: u32 = 0;
    let get = |action: SYSTEM_PARAMETERS_INFO_ACTION| {
        let mut value = 0u32;
        // SAFETY: each fills one UINT (or BOOL, the same size).
        unsafe {
            SystemParametersInfoW(
                action,
                0,
                Some(&mut value as *mut _ as *mut _),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .ok()
        .map(|_| value)
    };
    if get(SPI_GETFONTSMOOTHING)? == 0 || get(SPI_GETFONTSMOOTHINGTYPE)? != CLEARTYPE {
        return None;
    }
    Some(match get(SPI_GETFONTSMOOTHINGORIENTATION) {
        Some(BGR) => LcdOrder::Bgr,
        _ => LcdOrder::Rgb,
    })
}

#[cfg(not(windows))]
fn system() -> Option<LcdOrder> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_override_turns_lcd_text_off_or_picks_its_order() {
        assert_eq!(parse("0"), Some(None));
        assert_eq!(parse(" OFF "), Some(None));
        assert_eq!(parse("rgb"), Some(Some(LcdOrder::Rgb)));
        assert_eq!(parse("BGR"), Some(Some(LcdOrder::Bgr)));
        assert_eq!(parse("sideways"), None, "the system's setting then");
        for order in [None, Some(LcdOrder::Rgb), Some(LcdOrder::Bgr)] {
            assert_eq!(decode(encode(order)), order);
        }
    }
}
