//! The arrow styles offered by the arrow tool, laid out as a grid (8 rows of 7), and the small
//! pictures drawn for them.

use crate::doc::{Arrow, ArrowKind, ArrowStyle, BOND_LEN, Half, HeadSize};
use egui::{Color32, Painter, Pos2, Rect, pos2, vec2};

#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub kind: ArrowKind,
    pub style: ArrowStyle,
}

pub const COLS: usize = 7;

const SIZES: [HeadSize; 3] = [HeadSize::Large, HeadSize::Medium, HeadSize::Small];
const SWEEPS: [f32; 4] = [270.0, 180.0, 120.0, 90.0];

fn st(size: HeadSize) -> ArrowStyle {
    ArrowStyle { size, ..Default::default() }
}

/// Every style, row by row.
pub fn presets() -> Vec<Preset> {
    let mut v = Vec::new();
    let mut row = |left: &dyn Fn(HeadSize) -> Preset, right: Vec<Preset>| {
        for s in SIZES {
            v.push(left(s));
        }
        v.extend(right);
    };
    let curved = |sign: f32, half: Half, both: bool| -> Vec<Preset> {
        SWEEPS.iter().map(|&a| Preset { kind: ArrowKind::Curved, style: ArrowStyle { arc: a * sign, half, both, ..Default::default() } }).collect()
    };
    // 1: plain arrows; hollow block arrows; retrosynthetic arrows.
    row(
        &|s| Preset { kind: ArrowKind::Reaction, style: st(s) },
        vec![
            Preset { kind: ArrowKind::Hollow, style: st(HeadSize::Large) },
            Preset { kind: ArrowKind::Hollow, style: st(HeadSize::Small) },
            Preset { kind: ArrowKind::Retro, style: st(HeadSize::Large) },
            Preset { kind: ArrowKind::Retro, style: st(HeadSize::Small) },
        ],
    );
    // 2: wide heads; curved arrows bulging up.
    row(&|s| Preset { kind: ArrowKind::Reaction, style: ArrowStyle { size: s, wide: true, ..Default::default() } }, curved(1.0, Half::Full, false));
    // 3: dashed; curved arrows bulging down.
    row(&|s| Preset { kind: ArrowKind::Dashed, style: st(s) }, curved(-1.0, Half::Full, false));
    // 4: upper half heads; one-electron (fishhook) arrows bulging up.
    row(&|s| Preset { kind: ArrowKind::Reaction, style: ArrowStyle { size: s, half: Half::Left, ..Default::default() } }, curved(1.0, Half::Left, false));
    // 5: lower half heads; fishhooks bulging down.
    row(&|s| Preset { kind: ArrowKind::Reaction, style: ArrowStyle { size: s, half: Half::Right, ..Default::default() } }, curved(-1.0, Half::Right, false));
    // 6: resonance; curved arrows with a head at each end.
    row(&|s| Preset { kind: ArrowKind::Resonance, style: st(s) }, curved(1.0, Half::Full, true));
    // 7: crossed no-go arrows; equilibria; rotation one way.
    let mut r7: Vec<Preset> = SIZES.iter().map(|&s| Preset { kind: ArrowKind::Equilibrium, style: st(s) }).collect();
    r7.push(Preset { kind: ArrowKind::Rotation, style: ArrowStyle::default() });
    row(&|s| Preset { kind: ArrowKind::NoGo, style: st(s) }, r7);
    // 8: hashed no-go arrows; unbalanced equilibria; rotation the other way.
    let mut r8: Vec<Preset> = SIZES.iter().map(|&s| Preset { kind: ArrowKind::Unbalanced, style: st(s) }).collect();
    r8.push(Preset { kind: ArrowKind::Rotation, style: ArrowStyle { arc: -1.0, ..Default::default() } });
    row(&|s| Preset { kind: ArrowKind::NoGoHash, style: st(s) }, r8);
    v
}

fn size_name(s: HeadSize) -> &'static str {
    match s {
        HeadSize::Large => "large head",
        HeadSize::Medium => "medium head",
        HeadSize::Small => "small head",
    }
}

pub fn kind_name(k: ArrowKind) -> &'static str {
    match k {
        ArrowKind::Reaction => "Reaction arrow",
        ArrowKind::Line => "Plain line",
        ArrowKind::Equilibrium => "Equilibrium arrow",
        ArrowKind::Retro => "Retrosynthetic arrow",
        ArrowKind::Resonance => "Resonance arrow",
        ArrowKind::Curved => "Curved arrow",
        ArrowKind::Dashed => "Dashed arrow",
        ArrowKind::NoGo => "No-go arrow (crossed)",
        ArrowKind::Hollow => "Block arrow",
        ArrowKind::Unbalanced => "Unbalanced equilibrium",
        ArrowKind::NoGoHash => "No-go arrow (slashed)",
        ArrowKind::Rotation => "Rotation arrow",
    }
}

/// A plain-words description of a style, for tooltips.
pub fn describe(kind: ArrowKind, s: &ArrowStyle) -> String {
    let mut parts = vec![kind_name(kind).to_string()];
    match kind {
        ArrowKind::Curved => {
            if s.arc.abs() > 1.0 {
                parts.push(format!("{:.0}°", s.arc.abs()));
                parts.push(if s.arc > 0.0 { "bulging up".into() } else { "bulging down".into() });
            }
            if s.half != Half::Full {
                parts.push("half head (one electron)".into());
            } else {
                parts.push("full head (electron pair)".into());
            }
            if s.both {
                parts.push("heads at both ends".into());
            }
        }
        ArrowKind::Rotation => parts.push(if s.arc < 0.0 { "anticlockwise".into() } else { "clockwise".into() }),
        ArrowKind::Line => {}
        _ => {
            parts.push(size_name(s.size).into());
            if s.wide {
                parts.push("wide".into());
            }
            match s.half {
                Half::Left => parts.push("upper half head".into()),
                Half::Right => parts.push("lower half head".into()),
                Half::Full => {}
            }
        }
    }
    parts.join(", ")
}

/// A sample arrow of this style, sized to show well in a palette cell.
fn sample(kind: ArrowKind, style: ArrowStyle) -> Arrow {
    let l = BOND_LEN * 1.35;
    let (from, to) = match kind {
        ArrowKind::Rotation => (pos2(0.0, -l * 0.42), pos2(0.0, l * 0.42)),
        ArrowKind::Curved if style.arc.abs() >= 250.0 => (pos2(-l * 0.32, 0.0), pos2(l * 0.32, 0.0)),
        ArrowKind::Curved if style.arc.abs() >= 170.0 => (pos2(-l * 0.4, 0.0), pos2(l * 0.4, 0.0)),
        _ => (pos2(-l * 0.5, 0.0), pos2(l * 0.5, 0.0)),
    };
    Arrow::new(from, to, kind, style)
}

/// Draw a picture of an arrow style in `rect`.
pub fn icon(p: &Painter, rect: Rect, kind: ArrowKind, style: ArrowStyle, col: Color32) {
    let a = sample(kind, style);
    let mut b = Rect::NOTHING;
    for q in a.path(32) {
        b.extend_with(q);
    }
    let b = b.expand2(vec2(BOND_LEN * 0.12, BOND_LEN * 0.25));
    let zoom = ((rect.width() - 4.0) / b.width()).min((rect.height() - 2.0) / b.height()).min(1.1);
    let origin: Pos2 = rect.center() - b.center().to_vec2() * zoom;
    let mut cv = crate::render::EguiCanvas { painter: p, origin, zoom };
    crate::render::draw_arrow(&a, &mut cv, 1.3 / zoom, col);
}
