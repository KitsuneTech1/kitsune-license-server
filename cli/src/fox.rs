//! Route-B mascot: the Kitsune Technologies fox logo as ASCII, animated with
//! effects rather than sprite frames — typewriter draw-on, then an ember→gold
//! shimmer band that sweeps down the art while the server call is in flight.
//! Plain output (no ANSI, no animation) when not a TTY or KTSN_NO_ANIM is set.

use std::io::{IsTerminal, Write};
use std::time::Duration;

use crossterm::{
    cursor, execute, queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};

/// kitsunetech-site/public/logo.png → asciigen bold preset, width 46.
pub const FOX: &str = r#"            +#@..#@.
          .+@@@#+++
        .#@#@@@@@#+
  .@@@@@@###@@#+.     ..
  #@@###+++++++#++.  .++
 .@@@##++.++..     .#++.
.@@@#+. .#@+       +.++++
 #@@++  ##@@#+.    # +.+.
  .#@#+..##@@@@@##+ . +..
   #@@##@#@@@@@@@@#+   +#
    .+#######@@@@@@@+ .#@+
      .++##++++++++++#@#@+
   .+##@@@@@@@@@###@@@@@@
  .++##@@@@@@@@@@@@@@@@#.
   .++##@@@@@@@@@@@@@@#+
    ++..   ...++####++."#;

const INDENT: &str = "  ";

// Brand palette (site tokens): rust accent + ember/gold shimmer.
const RUST: Color = Color::Rgb { r: 193, g: 74, b: 40 };
const EMBER: Color = Color::Rgb { r: 232, g: 116, b: 58 };
const GOLD: Color = Color::Rgb { r: 240, g: 178, b: 82 };
const DIM: Color = Color::Rgb { r: 122, g: 66, b: 47 };

pub struct Mascot {
    pub enabled: bool,
    lines: Vec<String>,
}

fn base_color(c: char) -> Color {
    match c {
        '@' => EMBER,
        '#' => RUST,
        '+' => DIM,
        _ => DIM,
    }
}

impl Mascot {
    pub fn new() -> Self {
        let enabled = std::io::stdout().is_terminal()
            && std::env::var_os("KTSN_NO_ANIM").is_none()
            && std::env::var_os("NO_COLOR").is_none();
        let lines = FOX.lines().map(|l| l.to_string()).collect();
        Mascot { enabled, lines }
    }

    pub fn height(&self) -> u16 {
        self.lines.len() as u16
    }

    /// Typewriter draw-on: reveal the art in scanline order, colored.
    pub fn draw_in(&self) {
        let mut out = std::io::stdout();
        if !self.enabled {
            return; // plain runs skip the art entirely
        }
        let _ = execute!(out, cursor::Hide);
        let total: usize = self.lines.iter().map(|l| l.chars().count()).sum();
        let steps = 24u32;
        for step in 1..=steps {
            let reveal = (total as u32 * step / steps) as usize;
            let mut shown = 0usize;
            if step > 1 {
                let _ = queue!(out, cursor::MoveToPreviousLine(self.height()));
            }
            for line in &self.lines {
                let _ = queue!(out, Clear(ClearType::CurrentLine), Print(INDENT));
                for ch in line.chars() {
                    if shown < reveal && ch != ' ' {
                        let _ = queue!(out, SetForegroundColor(base_color(ch)), Print(ch));
                    } else {
                        let _ = queue!(out, Print(' '));
                    }
                    shown += 1;
                }
                let _ = queue!(out, ResetColor, Print("\r\n"));
            }
            let _ = out.flush();
            std::thread::sleep(Duration::from_millis(26));
        }
        let _ = execute!(out, cursor::Show);
    }

    /// One shimmer step: repaint the art with a bright band at `tick` sweeping
    /// downward. `lines_below` = how many lines the caller has printed since
    /// the art (prompt, status line, ...), so we can find our way back up.
    pub fn shimmer_step(&self, tick: usize, lines_below: u16) {
        if !self.enabled {
            return;
        }
        let mut out = std::io::stdout();
        let up = self.height() + lines_below;
        let band = (tick / 2) % (self.lines.len() + 6);
        let _ = queue!(out, cursor::Hide, cursor::SavePosition, cursor::MoveToPreviousLine(up));
        for (row, line) in self.lines.iter().enumerate() {
            let _ = queue!(out, Print(INDENT));
            let dist = (row as isize - band as isize).unsigned_abs();
            let color = match dist {
                0 => GOLD,
                1 => EMBER,
                _ => RUST,
            };
            for ch in line.chars() {
                if ch == ' ' {
                    let _ = queue!(out, Print(' '));
                } else {
                    let c = if dist <= 1 { color } else { base_color(ch) };
                    let _ = queue!(out, SetForegroundColor(c), Print(ch));
                }
            }
            let _ = queue!(out, ResetColor, Clear(ClearType::UntilNewLine), Print("\r\n"));
        }
        let _ = queue!(out, cursor::RestorePosition, cursor::Show);
        let _ = out.flush();
    }

    /// Final repaint in full brand color (steady, no band).
    pub fn settle(&self, lines_below: u16) {
        if !self.enabled {
            return;
        }
        let mut out = std::io::stdout();
        let up = self.height() + lines_below;
        let _ = queue!(out, cursor::SavePosition, cursor::MoveToPreviousLine(up));
        for line in &self.lines {
            let _ = queue!(out, Print(INDENT));
            for ch in line.chars() {
                if ch == ' ' {
                    let _ = queue!(out, Print(' '));
                } else {
                    let _ = queue!(out, SetForegroundColor(base_color(ch)), Print(ch));
                }
            }
            let _ = queue!(out, ResetColor, Clear(ClearType::UntilNewLine), Print("\r\n"));
        }
        let _ = queue!(out, cursor::RestorePosition);
        let _ = out.flush();
    }
}
