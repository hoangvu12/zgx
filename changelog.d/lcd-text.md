---
section: added
---
- **Text below 2x can be drawn for an LCD screen's subpixels, as ClearType draws it** (zgx-gpu, zgx-desktop): `GpuRenderer::set_lcd_text(Some(LcdOrder::Rgb))` draws each glyph's red, green and blue with their own subpixel's coverage, blended channel by channel (dual-source blending), three times as fine across as plain coverage. It applies only where it shows right: an opaque window's own target, below 2x, off macOS, on a device with dual-source blending; text in a translucent or frosted window or an isolated layer stays plain. On Windows the desktop host follows ClearType's setting (on, and its RGB or BGR order), read again on each settings change; `ZGX_LCD_TEXT=0`, `rgb` or `bgr` overrides it.
