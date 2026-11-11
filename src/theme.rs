//! Glowgrid looks: the three palettes and the stylesheet built from them.
//! Rules of the design system: dark world, square corners, hard offset shadows,
//! light only from the palette. https://github.com/maxmaster55/pixal_design_system

pub struct Palette {
    pub accent: &'static str,
    pub accent_hot: &'static str,
    pub glow_1: &'static str,
    pub glow_2: &'static str,
    pub glow_3: &'static str,
}

pub const NAMES: [&str; 3] = ["ember", "forge", "frost"];

pub fn palette(name: &str) -> Palette {
    match name {
        "forge" => Palette {
            accent: "#ff9a3c",
            accent_hot: "#ffc56a",
            glow_1: "#ffe08a",
            glow_2: "#f07a22",
            glow_3: "#bdf0ff",
        },
        "frost" => Palette {
            accent: "#4bc6e9",
            accent_hot: "#bdf0ff",
            glow_1: "#8fe3f7",
            glow_2: "#e6fbff",
            glow_3: "#ffd36b",
        },
        _ => Palette {
            accent: "#ff8a2a",
            accent_hot: "#ffd36b",
            glow_1: "#7cf36b",
            glow_2: "#e94bd6",
            glow_3: "#4bc6e9",
        },
    }
}

/// Read a theme name from text such as the Glowgrid state file; None if it is not one of ours.
pub fn parse_name(text: &str) -> Option<&'static str> {
    let t = text.trim();
    NAMES.iter().copied().find(|n| *n == t)
}

pub fn css(name: &str) -> String {
    let p = palette(name);
    TEMPLATE
        .replace("@ACCENT_HOT@", p.accent_hot)
        .replace("@ACCENT@", p.accent)
        .replace("@GLOW1@", p.glow_1)
        .replace("@GLOW2@", p.glow_2)
        .replace("@GLOW3@", p.glow_3)
}

// Shared dark-world colours are written literally: bg #0b0714, surface #170d24, air #1b1029,
// stone #3b2a3f, stone-lit #8a5a5a, text #e9def0, text-soft #a493b0, ink #1c1226, alert #ff5f5f.
const TEMPLATE: &str = r#"
* { border-radius: 0; box-shadow: none; text-shadow: none; outline-offset: 0; -gtk-icon-shadow: none; }

window.pixelscale { background: transparent; color: #e9def0; font-family: "IBM Plex Sans", "Noto Sans", sans-serif; font-size: 14px; }
window.pixelscale.plain { background: #170d24; }

.panel { background: #170d24; border: 2px solid @ACCENT@; }
.shadowed { margin: 0 8px 8px 0; box-shadow: 8px 8px 0 0 #000000; }

.head { background: #1b1029; padding: 16px 16px 14px 16px; border-bottom: 2px solid #000000; }
.state { font-family: "Pixelify Sans", "Silkscreen", monospace; font-weight: 700; font-size: 26px; color: @GLOW1@; }
.state.off { color: #8a5a5a; }
.state.warn { color: #ff5f5f; }
.sub { font-family: "VT323", monospace; font-size: 19px; color: #a493b0; }

/* switches: a square track and a square thumb */
switch { background: #3b2a3f; border: 2px solid #000000; min-width: 40px; min-height: 18px; padding: 2px; }
switch:checked { background: @ACCENT@; }
switch slider { background: #a493b0; min-width: 14px; min-height: 14px; border: none; margin: 0; }
switch:checked slider { background: #1c1226; }
switch:disabled { opacity: 0.5; }
switch.power { min-width: 60px; min-height: 26px; }
switch.power slider { min-width: 24px; min-height: 22px; }
switch.power:checked { background: @GLOW1@; }
switch:focus-visible, button:focus-visible { outline: 2px solid @GLOW3@; }

.tabs { background: #0b0714; border-bottom: 2px solid #000000; }
button.tab { background: transparent; color: #a493b0; border: none; border-top: 3px solid transparent; padding: 10px 4px 9px 4px;
  font-family: "Silkscreen", "Pixelify Sans", monospace; font-size: 10.5px; letter-spacing: 1.3px; min-height: 0; }
button.tab:hover { color: #e9def0; }
button.tab:checked { background: #170d24; color: @ACCENT_HOT@; border-top-color: @ACCENT@; }
button.tab:disabled { opacity: 0.45; }
.count { font-family: "VT323", monospace; font-size: 16px; letter-spacing: 0; color: #a493b0; }

.page { padding: 12px; }
scrolledwindow, viewport, stack, list, .page { background: transparent; }
scrollbar { background: transparent; border: none; }
scrollbar slider { background: #3b2a3f; min-width: 6px; min-height: 24px; border: none; margin: 2px; }
scrollbar slider:hover { background: #8a5a5a; }

.group { font-family: "Silkscreen", monospace; font-size: 10px; letter-spacing: 1.6px; color: @ACCENT@; padding: 6px 4px 0 4px; }
.row { background: #0b0714; border: 1px solid #3b2a3f; padding: 9px 10px; }
.row:hover, .row.me { border: 2px solid @ACCENT@; padding: 8px 9px; }
.row.drop-over { border: 2px solid @GLOW1@; padding: 8px 9px; }
.dot { background: #3b2a3f; min-width: 12px; min-height: 12px; }
.dot.on { background: @GLOW1@; }
.dot.exit { background: @GLOW2@; }
.name { font-weight: 600; font-size: 14.5px; }
.tag { font-family: "Silkscreen", monospace; font-size: 9.5px; letter-spacing: 1.2px; color: #a493b0; }
.tag.me { color: @ACCENT@; }
.addr { font-family: "VT323", monospace; font-size: 18px; color: @GLOW3@; }
.addr.dim { color: #a493b0; }
.hint { font-size: 12.5px; color: #a493b0; }

button.icon { background: #170d24; color: #a493b0; border: 1px solid #3b2a3f; min-width: 26px; min-height: 26px; padding: 0;
  margin: 0 3px 3px 0; box-shadow: 3px 3px 0 0 #000000; }
button.icon:hover { background: @ACCENT@; color: #1c1226; border-color: #000000; }
button.icon:active { margin: 3px 0 0 3px; box-shadow: none; }
button.icon:disabled { opacity: 0.4; }

button.text { font-family: "Silkscreen", monospace; font-size: 10.5px; letter-spacing: 1.2px; padding: 7px 12px; min-height: 0;
  background: #170d24; color: #e9def0; border: 1px solid #3b2a3f; margin: 0 4px 4px 0; box-shadow: 4px 4px 0 0 #000000; }
button.text:hover { background: @ACCENT@; color: #1c1226; border-color: #000000; }
button.text:active { margin: 4px 0 0 4px; box-shadow: none; }
button.text.primary { background: @ACCENT@; color: #1c1226; border-color: #000000; }
button.text:disabled { opacity: 0.45; }
.pill { font-family: "Silkscreen", monospace; font-size: 9.5px; letter-spacing: 1.2px; padding: 5px 8px; background: @ACCENT@; color: #1c1226; }

.dropzone { background: #0b0714; border: 2px dashed #3b2a3f; padding: 20px 12px; color: #a493b0; }
.dropzone.drop-over { border-color: @GLOW1@; }
.dropzone .big { font-family: "Pixelify Sans", monospace; font-weight: 600; font-size: 18px; color: #e9def0; }
.cell { background: #3b2a3f; min-height: 8px; }
.cell.full { background: @ACCENT@; }
.cell.done { background: @GLOW1@; }
.cell.failed { background: #ff5f5f; }
.meta { font-family: "VT323", monospace; font-size: 17px; color: #a493b0; }

dropdown > button { background: #0b0714; color: #e9def0; border: 1px solid #3b2a3f; padding: 6px 10px; min-height: 0; }
dropdown > button:hover { border-color: @ACCENT@; }
popover > contents { background: #170d24; border: 2px solid @ACCENT@; padding: 4px; color: #e9def0; }
popover > arrow { background: transparent; border: none; }
popover listview > row { padding: 6px 10px; }
popover listview > row:selected, popover listview > row:hover { background: @ACCENT@; color: #1c1226; }

.message { padding: 28px 20px; color: #a493b0; }
.message .big { font-family: "Pixelify Sans", monospace; font-weight: 600; font-size: 20px; color: #e9def0; }

.foot { background: #0b0714; border-top: 2px solid #000000; padding: 9px 14px; font-family: "VT323", monospace; font-size: 17px; color: #a493b0; }
.toast { color: @GLOW1@; }
.toast.bad { color: #ff5f5f; }
.good { color: @GLOW1@; }

/* ---- motion: short, and in held steps rather than smooth tweens ---- */
switch { transition: background-color 120ms steps(3); }
switch slider { transition: background-color 120ms steps(3); }
.row { transition: border-color 80ms steps(2); }
button.tab { transition: color 80ms steps(2), border-color 80ms steps(2), background-color 80ms steps(2); }
button.icon, button.text { transition: background-color 80ms steps(2), color 80ms steps(2); }
.cell { transition: background-color 120ms steps(2); }

/* lights breathe: two held levels, like a pixel-art glow */
@keyframes breathe { from { opacity: 1; } to { opacity: 0.55; } }
.dot.on, .dot.exit { animation: breathe 1.4s steps(2) infinite alternate; }
.state.warn { animation: breathe 0.8s steps(2) infinite alternate; }

/* rows rise into place when the panel opens, one after another */
@keyframes rise { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0); } }
.fresh .row, .fresh .group, .fresh .dropzone { animation: rise 200ms steps(5) both; }
.fresh .page > :nth-child(2) { animation-delay: 30ms; }
.fresh .page > :nth-child(3) { animation-delay: 60ms; }
.fresh .page > :nth-child(4) { animation-delay: 90ms; }
.fresh .page > :nth-child(5) { animation-delay: 120ms; }
.fresh .page > :nth-child(6) { animation-delay: 150ms; }
.fresh .page > :nth-child(7) { animation-delay: 180ms; }
.fresh .page > :nth-child(8) { animation-delay: 210ms; }
.fresh .page > :nth-child(n+9) { animation-delay: 240ms; }

/* the spark chase in the footer */
.spark { background: #3b2a3f; min-width: 6px; min-height: 6px; }
.spark.warm { background: @ACCENT@; }
.spark.hot { background: @ACCENT_HOT@; }
tooltip { background: #0b0714; border: 1px solid @ACCENT@; color: #e9def0; padding: 4px 8px; }
tooltip label { font-size: 12.5px; }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_placeholder_is_filled() {
        for n in NAMES {
            let c = css(n);
            assert!(
                !c.contains('@') || !c.contains("@ACCENT"),
                "{n} left a placeholder"
            );
            assert!(!c.contains("@GLOW"));
        }
    }

    #[test]
    fn theme_names() {
        assert_eq!(parse_name("frost\n"), Some("frost"));
        assert_eq!(parse_name("solarized"), None);
        assert_eq!(palette("nonsense").accent, palette("ember").accent);
    }
}
