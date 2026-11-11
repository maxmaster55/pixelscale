//! One-bit pixel icons on a 7x7 grid, drawn in the widget's current text colour,
//! and the tray icon lattice. No image files, so nothing can fail to load.

use gtk::prelude::*;

pub const COPY: [&str; 7] = [
    "0011111", "0010001", "1110001", "1010001", "1011111", "1000100", "1111100",
];
pub const TERMINAL: [&str; 7] = [
    "0000000", "1000000", "0100000", "0010000", "0100000", "1000111", "0000000",
];
pub const PING: [&str; 7] = [
    "0000000", "0010000", "0010000", "1101011", "0000100", "0000100", "0000000",
];
pub const SEND: [&str; 7] = [
    "0001000", "0011100", "0101010", "0001000", "0001000", "0001000", "0001000",
];
pub const SAVE: [&str; 7] = [
    "0001000", "0001000", "0001000", "0101010", "0011100", "0001000", "1111111",
];
pub const CLOSE: [&str; 7] = [
    "1000001", "0100010", "0010100", "0001000", "0010100", "0100010", "1000001",
];

/// A 7x7 icon scaled by `cell` pixels per art pixel.
pub fn icon(bits: &'static [&'static str; 7], cell: i32) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(7 * cell);
    area.set_content_height(7 * cell);
    area.set_halign(gtk::Align::Center);
    area.set_valign(gtk::Align::Center);
    area.set_draw_func(move |w, cr, _, _| {
        let c = w.color();
        cr.set_source_rgba(
            c.red() as f64,
            c.green() as f64,
            c.blue() as f64,
            c.alpha() as f64,
        );
        for (j, row) in bits.iter().enumerate() {
            for (i, b) in row.bytes().enumerate() {
                if b == b'1' {
                    cr.rectangle(
                        (i as i32 * cell) as f64,
                        (j as i32 * cell) as f64,
                        cell as f64,
                        cell as f64,
                    );
                }
            }
        }
        let _ = cr.fill();
    });
    area
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrayState {
    Connected,
    ExitNode,
    Off,
    NeedsLogin,
}

fn rgb(hex: &str) -> (u8, u8, u8) {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    ((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// The tray icon: a 3x3 lattice of cells with a "T" lit when connected. Returns
/// (size, ARGB32 bytes in network byte order), the format StatusNotifierItem wants.
pub fn tray_pixmap(state: TrayState, theme: &str, scale: usize) -> (i32, Vec<u8>) {
    let p = crate::theme::palette(theme);
    let lit = rgb("#e9def0");
    let unlit = rgb("#3b2a3f");
    let mark = match state {
        TrayState::ExitNode => Some(rgb(p.glow_2)),
        TrayState::NeedsLogin => Some(rgb("#ff5f5f")),
        _ => None,
    };
    let on = matches!(
        state,
        TrayState::Connected | TrayState::ExitNode | TrayState::NeedsLogin
    );
    let size = 16 * scale;
    let mut px = vec![0u8; size * size * 4];
    for j in 0..3 {
        for i in 0..3 {
            let in_t = on && (j == 1 || (j == 2 && i == 1));
            let colour = match mark {
                Some(m) if j == 0 && i == 2 => m,
                _ if in_t => lit,
                _ => unlit,
            };
            for y in (1 + j * 5) * scale..(1 + j * 5 + 4) * scale {
                for x in (1 + i * 5) * scale..(1 + i * 5 + 4) * scale {
                    let k = (y * size + x) * 4;
                    px[k] = 255;
                    px[k + 1] = colour.0;
                    px[k + 2] = colour.1;
                    px[k + 3] = colour.2;
                }
            }
        }
    }
    (size as i32, px)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_are_seven_by_seven() {
        for bits in [COPY, TERMINAL, PING, SEND, SAVE, CLOSE] {
            assert!(bits
                .iter()
                .all(|r| r.len() == 7 && r.bytes().all(|b| b == b'0' || b == b'1')));
        }
    }

    #[test]
    fn tray_pixmap_shape() {
        let (size, px) = tray_pixmap(TrayState::Connected, "ember", 2);
        assert_eq!(size, 32);
        assert_eq!(px.len(), 32 * 32 * 4);
        // top-left cell is unlit stone, centre cell is lit, the gutter is transparent
        let at = |x: usize, y: usize| &px[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4];
        assert_eq!(at(3, 3), &[255, 0x3b, 0x2a, 0x3f]);
        assert_eq!(at(15, 15), &[255, 0xe9, 0xde, 0xf0]);
        assert_eq!(at(0, 0), &[0, 0, 0, 0]);
        let (_, off) = tray_pixmap(TrayState::Off, "ember", 1);
        assert_eq!(
            &off[(7 * 16 + 7) * 4..(7 * 16 + 7) * 4 + 4],
            &[255, 0x3b, 0x2a, 0x3f]
        );
    }
}
