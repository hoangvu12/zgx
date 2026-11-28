//! Text drawn for an LCD screen's subpixels (`set_lcd_text`), as ClearType
//! draws it: below 2x, into an opaque window's target, a glyph's edges take
//! the colour of the subpixels they cover (a white stem's left edge blue on
//! an RGB screen, its right edge red); anywhere it wouldn't show right (a
//! translucent window, an isolated layer, 2x), text stays plain grey.
use zgx::scene::*;
use zgx_gpu::{GpuRenderer, LcdOrder, text::FontData};

const GEIST: &[u8] = include_bytes!("fonts/GeistVariable-subset.ttf");
const GROUND: Color = Color(20, 20, 20, 255);
const W: f32 = 200.;
const H: f32 = 30.;

fn renderer(scale: f32, order: Option<LcdOrder>) -> GpuRenderer {
    let mut gpu = GpuRenderer::new((W * scale) as u32, (H * scale) as u32).expect("a GPU adapter");
    gpu.set_scale_factor(scale);
    gpu.set_lcd_text(order);
    FontData::new(GEIST.to_vec())
        .unwrap()
        .install(&mut gpu.text_system().borrow_mut());
    gpu
}

/// `text` in white Geist over `GROUND`, isolated in a layer if `isolated`.
fn scene(gpu: &GpuRenderer, text: &str, isolated: bool) -> Scene {
    let mut scene = Scene::new(W, H);
    gpu.install_text(&mut scene);
    let node = scene.append(
        scene.root(),
        NodeKind::Text {
            text: text.into(),
            font_size: 16.,
            color: Color(255, 255, 255, 255),
        },
        Style {
            width: Some(W),
            height: Some(H),
            ..Default::default()
        },
    );
    scene.set_font(
        node,
        zgx::text_layout::FontStyle {
            family: "Geist".into(),
            ..Default::default()
        },
    );
    if isolated {
        scene.set_isolated(node, true);
    }
    scene
}

/// The pixels (premultiplied RGBA) of `scene` drawn by `gpu` over `ground`.
fn draw(gpu: &mut GpuRenderer, scene: &mut Scene, ground: Color) -> Vec<[u8; 4]> {
    gpu.set_background(ground);
    let damage = scene.flush().damage;
    gpu.render(scene, &damage).unwrap();
    gpu.readback().unwrap().as_chunks::<4>().0.to_vec()
}

/// Pixels whose red and blue stand apart: coloured edges.
fn coloured(pixels: &[[u8; 4]]) -> usize {
    pixels
        .iter()
        .filter(|p| (p[0] as i32 - p[2] as i32).abs() > 8)
        .count()
}

/// The first inked pixel of each row of a lone stem, from the left: its
/// red less its blue, summed (negative: blue edges, as an RGB screen's).
fn left_edge_tint(pixels: &[[u8; 4]], width: usize) -> i32 {
    pixels
        .chunks(width)
        .filter_map(|row| row.iter().find(|p| p[1] > GROUND.1 + 24))
        .map(|p| p[0] as i32 - p[2] as i32)
        .sum()
}

/// Whether this device draws LCD text at all (dual-source blending): where
/// it can't, `set_lcd_text` takes nothing and text is plain everywhere.
fn lcd_drawn() -> bool {
    let gpu = renderer(1., Some(LcdOrder::Rgb));
    let drawn = gpu.lcd_text().is_some();
    if !drawn {
        eprintln!("skipped: no dual-source blending on this adapter");
    }
    drawn
}

#[cfg(not(target_os = "macos"))]
#[test]
fn an_opaque_windows_text_takes_its_subpixels_colours() {
    if !lcd_drawn() {
        return;
    }
    let mut plain = renderer(1., None);
    let pixels = {
        let mut s = scene(&plain, "minimum illicit", false);
        draw(&mut plain, &mut s, GROUND)
    };
    assert_eq!(coloured(&pixels), 0, "plain text is grey");

    let mut lcd = renderer(1., Some(LcdOrder::Rgb));
    let pixels = {
        let mut s = scene(&lcd, "minimum illicit", false);
        draw(&mut lcd, &mut s, GROUND)
    };
    assert!(
        coloured(&pixels) > 40,
        "{} coloured pixels",
        coloured(&pixels)
    );
    assert!(
        pixels.iter().all(|p| p[3] == 255),
        "the target stays opaque"
    );
}

#[cfg(not(target_os = "macos"))]
#[test]
fn a_stems_left_edge_is_blue_on_rgb_and_red_on_bgr() {
    if !lcd_drawn() {
        return;
    }
    let width = W as usize;
    let mut rgb = renderer(1., Some(LcdOrder::Rgb));
    let pixels = {
        let mut s = scene(&rgb, "l", false);
        draw(&mut rgb, &mut s, GROUND)
    };
    let tint = left_edge_tint(&pixels, width);
    assert!(tint < -40, "RGB: {tint}");
    let mut bgr = renderer(1., Some(LcdOrder::Bgr));
    let pixels = {
        let mut s = scene(&bgr, "l", false);
        draw(&mut bgr, &mut s, GROUND)
    };
    let tint = left_edge_tint(&pixels, width);
    assert!(tint > 40, "BGR: {tint}");
}

/// A translucent window's pixels are blended again by the compositor, over
/// whatever lies behind (frosted glass among it): its text stays grey.
#[test]
fn a_translucent_windows_text_stays_plain() {
    let mut gpu = renderer(1., Some(LcdOrder::Rgb));
    let mut s = scene(&gpu, "minimum illicit", false);
    let pixels = draw(&mut gpu, &mut s, Color(20, 20, 20, 200));
    assert_eq!(coloured(&pixels), 0);
}

/// An isolated layer's pixels are composited again: its text stays grey,
/// and text outside it, drawn after, keeps its colours.
#[test]
fn text_in_a_layer_stays_plain() {
    let mut gpu = renderer(1., Some(LcdOrder::Rgb));
    let pixels = {
        let mut s = scene(&gpu, "minimum illicit", true);
        draw(&mut gpu, &mut s, GROUND)
    };
    assert_eq!(coloured(&pixels), 0);
}

#[test]
fn text_at_2x_stays_plain() {
    let mut gpu = renderer(2., Some(LcdOrder::Rgb));
    let pixels = {
        let mut s = scene(&gpu, "minimum illicit", false);
        draw(&mut gpu, &mut s, GROUND)
    };
    assert_eq!(coloured(&pixels), 0);
}

/// A window on Windows presents in a single pass (a frame drawn into the
/// drawable and the retained target at once), which LCD glyphs' dual-source
/// blending can't write: their frames are drawn into the target and
/// presented after, and the text shown keeps its subpixels' colours, frame
/// after frame. (It was plain grey there: LCD text turned itself off
/// wherever a single pass could present, every Windows window.)
#[cfg(not(target_os = "macos"))]
#[test]
fn a_single_pass_windows_text_takes_its_subpixels_colours() {
    if !lcd_drawn() {
        return;
    }
    let mut gpu = renderer(1., Some(LcdOrder::Rgb));
    gpu.debug_enable_single_pass();
    let mut s = scene(&gpu, "minimum illicit", false);
    gpu.set_background(GROUND);
    for text in ["minimum illicit", "illicit minimum"] {
        let node = s.children(s.root())[0];
        s.set_text(node, text);
        let damage = s.flush().damage;
        gpu.render(&mut s, &damage).unwrap();
        let shown = gpu.debug_present_offscreen().unwrap();
        let pixels = shown.as_chunks::<4>().0;
        assert!(coloured(pixels) > 40, "{text:?}: {} coloured pixels shown", coloured(pixels));
    }
}

/// Turned off, the next frame's text is grey again, and back on, coloured:
/// each text's glyphs are keyed again, not drawn from the last setting's.
#[cfg(not(target_os = "macos"))]
#[test]
fn turning_lcd_text_off_and_on_draws_it_again() {
    if !lcd_drawn() {
        return;
    }
    let mut gpu = renderer(1., Some(LcdOrder::Rgb));
    let mut scene = scene(&gpu, "minimum illicit", false);
    assert!(coloured(&draw(&mut gpu, &mut scene, GROUND)) > 40);
    gpu.set_lcd_text(None);
    assert_eq!(coloured(&draw(&mut gpu, &mut scene, GROUND)), 0);
    gpu.set_lcd_text(Some(LcdOrder::Rgb));
    assert!(coloured(&draw(&mut gpu, &mut scene, GROUND)) > 40);
}
