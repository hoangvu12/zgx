use super::*;
use zgx::scene::{Color, Rect};
/// A board of rounded cards, each clipped, at fractional points: wider and
/// taller than the box.
fn board(shade: u8) -> Arc<Canvas> {
    let mut canvas = Canvas::new();
    for row in 0..9 {
        for col in 0..12 {
            let (x, y) = (col as f32 * 157.3 + 3.7, row as f32 * 131.9 + 5.2);
            let card = Rect::new(x, y, 120.4, 101.7);
            canvas.clip(Path::rounded_rect(card, 9.5).unwrap(), FillRule::Winding);
            canvas.fill(
                Path::rounded_rect(card, 9.5).unwrap(),
                Color(shade, (col * 20) as u8, (row * 25) as u8, 255),
            );
            canvas.fill(
                Path::rounded_rect(Rect::new(x + 10.3, y + 60.1, 140., 20.6), 4.).unwrap(),
                Color(20, 30, 200, 180),
            );
            canvas.restore();
        }
    }
    Arc::new(canvas)
}
const W: f32 = 700.;
const H: f32 = 450.;
/// What `out` shows, unscaled, put together into one image of the box.
fn composed(out: &[(Raster, Placement)], scale: f32) -> Vec<u8> {
    let size = [(W * scale).ceil() as usize, (H * scale).ceil() as usize];
    let mut image = vec![0u8; size[0] * size[1] * 4];
    for (raster, placement) in out {
        assert!(!placement.scaled, "every tile of the zoom shown");
        let pixels = raster.pixels.as_ref().unwrap();
        let [x, y, w, h] = placement.rect.map(|v| (v * scale).round() as usize);
        let (u, v) = (
            (placement.uv[0] * TILE as f32).round() as usize,
            (placement.uv[1] * TILE as f32).round() as usize,
        );
        for row in 0..h.min(size[1] - y) {
            for col in 0..w.min(size[0] - x) {
                let src = ((v + row) * TILE as usize + u + col) * 4;
                let dst = ((y + row) * size[0] + x + col) * 4;
                image[dst..dst + 4].copy_from_slice(&pixels.pixels()[src..src + 4]);
            }
        }
    }
    image
}
/// What the box's raster shows of `canvas` drawn under `drawn`, panned by
/// `shift` device pixels.
fn raster(canvas: &Arc<Canvas>, drawn: View, shift: [i32; 2], scale: f32) -> Vec<u8> {
    let size = [(W * scale).ceil() as i32, (H * scale).ceil() as i32];
    let text = SharedText::default();
    let painted = panned_region(canvas, drawn, size, scale, None).painted;
    let seen = [-shift[0], -shift[1], size[0] - shift[0], size[1] - shift[1]];
    // With a margin, cut away: the raster's own edge is antialiased a
    // little differently (as each tile's would be, without theirs).
    let m = MARGIN;
    let grown = [seen[0] - m, seen[1] - m, seen[2] + m, seen[3] + m];
    let image = rasterize_region(canvas, drawn, scale, grown, None, &text).unwrap();
    let stride = (size[0] + 2 * m) as usize;
    let mut pixels = vec![0u8; (size[0] * size[1] * 4) as usize];
    for (row, line) in pixels.chunks_exact_mut(size[0] as usize * 4).enumerate() {
        let src = ((row + m as usize) * stride + m as usize) * 4;
        line.copy_from_slice(&image.pixels()[src..src + line.len()]);
    }
    // Outside what it paints, nothing (as the box's raster leaves it).
    for y in 0..size[1] {
        for x in 0..size[0] {
            let (dx, dy) = (x + seen[0], y + seen[1]);
            if dx < painted[0] || dy < painted[1] || dx >= painted[2] || dy >= painted[3] {
                let i = (y * size[0] + x) as usize * 4;
                pixels[i..i + 4].fill(0);
            }
        }
    }
    pixels
}
/// Whether `a` and `b` are the same pixels, but for antialiasing rounded
/// the other way here and there: tiny-skia's coverage depends a little on
/// where a pixmap starts (as two strips' of the box's raster do).
fn same(a: &[u8], b: &[u8], scale: f32) -> bool {
    let premultiplied = |p: &[u8]| {
        p[..3]
            .iter()
            .map(|c| *c as u32 * p[3] as u32 / 255)
            .collect::<Vec<_>>()
    };
    let (mut count, mut most) = (0, 0);
    for (x, y) in a.chunks(4).zip(b.chunks(4)) {
        if x == y {
            continue;
        }
        let (px, py) = (premultiplied(x), premultiplied(y));
        let d = (px.iter().zip(&py).map(|(p, q)| p.abs_diff(*q)))
            .chain([(x[3] as u32).abs_diff(y[3] as u32)])
            .max()
            .unwrap();
        count += 1;
        most = most.max(d);
    }
    let area = (W * scale).ceil() * (H * scale).ceil();
    if count as f32 > area / 20_000. || most > 32 {
        eprintln!("{count} pixels differ, by up to {most}");
        return false;
    }
    true
}
/// Tiles until none is awaited.
fn settled(
    cache: &mut CanvasCache,
    node: NodeId,
    canvas: &Arc<Canvas>,
    view: View,
    scale: f32,
) -> Vec<(Raster, Placement)> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let mut out = Vec::new();
        let awaiting = (cache.get_tiles(node, canvas, view, W, H, scale, false, &mut out)).unwrap();
        if !awaiting {
            return out;
        }
        assert!(Instant::now() < deadline, "tiles land");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn node() -> NodeId {
    let mut scene = Scene::new(100., 100.);
    let root = scene.root();
    scene.append(
        root,
        NodeKind::Canvas(Arc::new(Canvas::new())),
        Default::default(),
    )
}
fn view(x: f32, y: f32, scale: f32) -> View {
    View { x, y, scale }
}
#[test]
fn a_pan_past_the_raster_draws_tiles_off_the_frame_matching_the_raster() {
    for scale in [1., 1.5] {
        let canvas = board(200);
        let node = node();
        let mut cache = CanvasCache::default();
        let first = view(-40.3, -25.7, 1.);
        cache
            .get_panned(node, &canvas, first, W, H, scale, false)
            .unwrap();
        assert!(!cache.tiled(node));
        // Past it: tiled from now on, nothing rasterized on the frame.
        let panned = view(-640.3, -425.2, 1.);
        let (_, none) = cache
            .get_panned(node, &canvas, panned, W, H, scale, false)
            .unwrap();
        assert_eq!(none.rect[2] * none.rect[3], 0.);
        assert!(cache.tiled(node));
        let mut out = Vec::new();
        let begun = Instant::now();
        (cache.get_tiles(node, &canvas, panned, W, H, scale, false, &mut out)).unwrap();
        assert!(
            begun.elapsed() < Duration::from_millis(50),
            "nothing made on the frame"
        );
        let out = settled(&mut cache, node, &canvas, panned, scale);
        let shift = [
            ((panned.x - first.x) * scale).round() as i32,
            ((panned.y - first.y) * scale).round() as i32,
        ];
        let (tiles, raster) = (composed(&out, scale), raster(&canvas, first, shift, scale));
        assert!(
            same(&tiles, &raster, scale),
            "tiles at rest are the raster's pixels (scale {scale})"
        );
        if scale == 1. {
            assert!(tiles == raster, "at whole pixels, exactly");
        }
    }
}
#[test]
fn a_zoom_shows_another_level_scaled_then_its_own_tiles_matching_its_raster() {
    let canvas = board(120);
    let node = node();
    let mut cache = CanvasCache::default();
    let first = view(-10., -10., 1.);
    cache
        .get_panned(node, &canvas, first, W, H, 1., false)
        .unwrap();
    let zoomed = view(-300.25, -180.5, 1.7);
    cache
        .get_panned(node, &canvas, zoomed, W, H, 1., false)
        .unwrap();
    assert!(cache.tiled(node));
    let mut out = Vec::new();
    let awaiting = (cache.get_tiles(node, &canvas, zoomed, W, H, 1., false, &mut out)).unwrap();
    assert!(awaiting);
    assert!(
        out.iter().all(|(_, p)| p.scaled),
        "the zoom before stands in"
    );
    let out = settled(&mut cache, node, &canvas, zoomed, 1.);
    assert!(same(
        &composed(&out, 1.),
        &raster(&canvas, zoomed, [0, 0], 1.),
        1.
    ));
    // Back at the first zoom: its level shown at once (what it lacks from
    // the raster it was cut from made off the frame).
    let mut out = Vec::new();
    (cache.get_tiles(node, &canvas, first, W, H, 1., false, &mut out)).unwrap();
    let out = settled(&mut cache, node, &canvas, first, 1.);
    assert!(same(
        &composed(&out, 1.),
        &raster(&canvas, first, [0, 0], 1.),
        1.
    ));
}
#[test]
fn a_changed_drawing_shows_at_once_and_tiles_keep_to_their_budget() {
    let canvas = board(60);
    let node = node();
    let mut cache = CanvasCache::default();
    let mut at = view(0., 0., 1.);
    cache
        .get_panned(node, &canvas, at, W, H, 2., false)
        .unwrap();
    at.x -= 900.;
    cache
        .get_panned(node, &canvas, at, W, H, 2., false)
        .unwrap();
    settled(&mut cache, node, &canvas, at, 2.);
    let changed = board(250);
    let mut out = Vec::new();
    let awaiting = (cache.get_tiles(node, &changed, at, W, H, 2., false, &mut out)).unwrap();
    assert!(!awaiting, "a change is drawn before it shows");
    assert!(same(
        &composed(&out, 2.),
        &raster(&changed, at, [0, 0], 2.),
        2.
    ));
    // Across the whole board and back, at several zooms.
    for step in 0..40 {
        let k = [1., 0.5, 2., 0.25][step / 10];
        let v = view(-((step % 10) as f32) * 180., -((step % 7) as f32) * 120., k);
        settled(&mut cache, node, &changed, v, 2.);
        let p = &cache.pyramids[&node];
        assert!(p.bytes <= TILE_BUDGET.max(p.shown.len() * 3 * TILE_BYTES));
        assert!(p.levels.len() <= MAX_LEVELS);
    }
    assert!(
        !cache.take_retired().is_empty(),
        "evicted tiles' textures go"
    );
}
