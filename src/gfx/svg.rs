//! Minimal SVG path data parser and D2D PathGeometry builder.
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows_numerics::Vector2;

#[derive(Clone, Debug, PartialEq)]
pub enum Seg {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    BezierTo(f32, f32, f32, f32, f32, f32),
    QuadTo(f32, f32, f32, f32),
    ArcTo {
        rx: f32,
        ry: f32,
        rot: f32,
        large: bool,
        sweep: bool,
        x: f32,
        y: f32,
    },
    Close,
}

pub struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
        }
    }

    fn skip_whitespace_and_commas(&mut self) {
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if b.is_ascii_whitespace() || b == b',' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    pub fn next_cmd(&mut self) -> Option<char> {
        self.skip_whitespace_and_commas();
        if self.pos >= self.bytes.len() {
            return None;
        }
        let b = self.bytes[self.pos];
        if b.is_ascii_alphabetic() {
            self.pos += 1;
            Some(b as char)
        } else {
            None
        }
    }

    pub fn next_number(&mut self) -> Option<f32> {
        self.skip_whitespace_and_commas();
        if self.pos >= self.bytes.len() {
            return None;
        }
        let start = self.pos;
        let mut b = self.bytes[self.pos];
        if b == b'+' || b == b'-' {
            self.pos += 1;
            if self.pos >= self.bytes.len() {
                self.pos = start;
                return None;
            }
            b = self.bytes[self.pos];
        }

        let mut has_digits = false;
        let mut has_dot = false;
        while self.pos < self.bytes.len() {
            let c = self.bytes[self.pos];
            if c.is_ascii_digit() {
                has_digits = true;
                self.pos += 1;
            } else if c == b'.' && !has_dot {
                has_dot = true;
                self.pos += 1;
            } else {
                break;
            }
        }

        if !has_digits {
            self.pos = start;
            return None;
        }

        // Optional exponent e.g. 1e-4
        if self.pos < self.bytes.len() && (self.bytes[self.pos] == b'e' || self.bytes[self.pos] == b'E') {
            let e_start = self.pos;
            self.pos += 1;
            if self.pos < self.bytes.len() && (self.bytes[self.pos] == b'+' || self.bytes[self.pos] == b'-') {
                self.pos += 1;
            }
            let mut e_digits = false;
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                e_digits = true;
                self.pos += 1;
            }
            if !e_digits {
                self.pos = e_start;
            }
        }

        let s = &self.src[start..self.pos];
        s.parse::<f32>().ok()
    }

    pub fn next_flag(&mut self) -> Option<bool> {
        self.skip_whitespace_and_commas();
        if self.pos >= self.bytes.len() {
            return None;
        }
        let b = self.bytes[self.pos];
        if b == b'0' {
            self.pos += 1;
            Some(false)
        } else if b == b'1' {
            self.pos += 1;
            Some(true)
        } else {
            self.next_number().map(|n| n != 0.0)
        }
    }
}

pub fn parse_svg_path(d: &str) -> Vec<Seg> {
    let mut lex = Lexer::new(d);
    let mut segs = Vec::new();
    let mut curr_x = 0.0f32;
    let mut curr_y = 0.0f32;
    let mut start_x = 0.0f32;
    let mut start_y = 0.0f32;
    let mut last_ctrl_x = 0.0f32;
    let mut last_ctrl_y = 0.0f32;
    let mut last_cmd = ' ';

    while let Some(cmd) = lex.next_cmd().or_else(|| {
        if last_cmd != ' ' && last_cmd != 'Z' && last_cmd != 'z' {
            if last_cmd == 'M' {
                Some('L')
            } else if last_cmd == 'm' {
                Some('l')
            } else {
                Some(last_cmd)
            }
        } else {
            None
        }
    }) {
        match cmd {
            'M' => {
                if let (Some(x), Some(y)) = (lex.next_number(), lex.next_number()) {
                    curr_x = x;
                    curr_y = y;
                    start_x = x;
                    start_y = y;
                    segs.push(Seg::MoveTo(x, y));
                    last_cmd = 'M';
                }
            }
            'm' => {
                if let (Some(dx), Some(dy)) = (lex.next_number(), lex.next_number()) {
                    curr_x += dx;
                    curr_y += dy;
                    start_x = curr_x;
                    start_y = curr_y;
                    segs.push(Seg::MoveTo(curr_x, curr_y));
                    last_cmd = 'm';
                }
            }
            'L' => {
                while let (Some(x), Some(y)) = (lex.next_number(), lex.next_number()) {
                    curr_x = x;
                    curr_y = y;
                    segs.push(Seg::LineTo(x, y));
                    last_cmd = 'L';
                }
            }
            'l' => {
                while let (Some(dx), Some(dy)) = (lex.next_number(), lex.next_number()) {
                    curr_x += dx;
                    curr_y += dy;
                    segs.push(Seg::LineTo(curr_x, curr_y));
                    last_cmd = 'l';
                }
            }
            'H' => {
                while let Some(x) = lex.next_number() {
                    curr_x = x;
                    segs.push(Seg::LineTo(curr_x, curr_y));
                    last_cmd = 'H';
                }
            }
            'h' => {
                while let Some(dx) = lex.next_number() {
                    curr_x += dx;
                    segs.push(Seg::LineTo(curr_x, curr_y));
                    last_cmd = 'h';
                }
            }
            'V' => {
                while let Some(y) = lex.next_number() {
                    curr_y = y;
                    segs.push(Seg::LineTo(curr_x, curr_y));
                    last_cmd = 'V';
                }
            }
            'v' => {
                while let Some(dy) = lex.next_number() {
                    curr_y += dy;
                    segs.push(Seg::LineTo(curr_x, curr_y));
                    last_cmd = 'v';
                }
            }
            'C' => {
                while let (Some(x1), Some(y1), Some(x2), Some(y2), Some(x), Some(y)) = (
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_number(),
                ) {
                    segs.push(Seg::BezierTo(x1, y1, x2, y2, x, y));
                    last_ctrl_x = x2;
                    last_ctrl_y = y2;
                    curr_x = x;
                    curr_y = y;
                    last_cmd = 'C';
                }
            }
            'c' => {
                while let (Some(dx1), Some(dy1), Some(dx2), Some(dy2), Some(dx), Some(dy)) = (
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_number(),
                ) {
                    let x1 = curr_x + dx1;
                    let y1 = curr_y + dy1;
                    let x2 = curr_x + dx2;
                    let y2 = curr_y + dy2;
                    let x = curr_x + dx;
                    let y = curr_y + dy;
                    segs.push(Seg::BezierTo(x1, y1, x2, y2, x, y));
                    last_ctrl_x = x2;
                    last_ctrl_y = y2;
                    curr_x = x;
                    curr_y = y;
                    last_cmd = 'c';
                }
            }
            'S' => {
                while let (Some(x2), Some(y2), Some(x), Some(y)) = (
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_number(),
                ) {
                    let (x1, y1) = if matches!(last_cmd, 'C' | 'c' | 'S' | 's') {
                        (2.0 * curr_x - last_ctrl_x, 2.0 * curr_y - last_ctrl_y)
                    } else {
                        (curr_x, curr_y)
                    };
                    segs.push(Seg::BezierTo(x1, y1, x2, y2, x, y));
                    last_ctrl_x = x2;
                    last_ctrl_y = y2;
                    curr_x = x;
                    curr_y = y;
                    last_cmd = 'S';
                }
            }
            's' => {
                while let (Some(dx2), Some(dy2), Some(dx), Some(dy)) = (
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_number(),
                ) {
                    let (x1, y1) = if matches!(last_cmd, 'C' | 'c' | 'S' | 's') {
                        (2.0 * curr_x - last_ctrl_x, 2.0 * curr_y - last_ctrl_y)
                    } else {
                        (curr_x, curr_y)
                    };
                    let x2 = curr_x + dx2;
                    let y2 = curr_y + dy2;
                    let x = curr_x + dx;
                    let y = curr_y + dy;
                    segs.push(Seg::BezierTo(x1, y1, x2, y2, x, y));
                    last_ctrl_x = x2;
                    last_ctrl_y = y2;
                    curr_x = x;
                    curr_y = y;
                    last_cmd = 's';
                }
            }
            'Q' => {
                while let (Some(x1), Some(y1), Some(x), Some(y)) = (
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_number(),
                ) {
                    segs.push(Seg::QuadTo(x1, y1, x, y));
                    last_ctrl_x = x1;
                    last_ctrl_y = y1;
                    curr_x = x;
                    curr_y = y;
                    last_cmd = 'Q';
                }
            }
            'q' => {
                while let (Some(dx1), Some(dy1), Some(dx), Some(dy)) = (
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_number(),
                ) {
                    let x1 = curr_x + dx1;
                    let y1 = curr_y + dy1;
                    let x = curr_x + dx;
                    let y = curr_y + dy;
                    segs.push(Seg::QuadTo(x1, y1, x, y));
                    last_ctrl_x = x1;
                    last_ctrl_y = y1;
                    curr_x = x;
                    curr_y = y;
                    last_cmd = 'q';
                }
            }
            'A' => {
                while let (Some(rx), Some(ry), Some(rot), Some(large), Some(sweep), Some(x), Some(y)) = (
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_flag(),
                    lex.next_flag(),
                    lex.next_number(),
                    lex.next_number(),
                ) {
                    segs.push(Seg::ArcTo {
                        rx: rx.abs(),
                        ry: ry.abs(),
                        rot,
                        large,
                        sweep,
                        x,
                        y,
                    });
                    curr_x = x;
                    curr_y = y;
                    last_cmd = 'A';
                }
            }
            'a' => {
                while let (Some(rx), Some(ry), Some(rot), Some(large), Some(sweep), Some(dx), Some(dy)) = (
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_number(),
                    lex.next_flag(),
                    lex.next_flag(),
                    lex.next_number(),
                    lex.next_number(),
                ) {
                    let x = curr_x + dx;
                    let y = curr_y + dy;
                    segs.push(Seg::ArcTo {
                        rx: rx.abs(),
                        ry: ry.abs(),
                        rot,
                        large,
                        sweep,
                        x,
                        y,
                    });
                    curr_x = x;
                    curr_y = y;
                    last_cmd = 'a';
                }
            }
            'Z' | 'z' => {
                segs.push(Seg::Close);
                curr_x = start_x;
                curr_y = start_y;
                last_cmd = 'z';
            }
            _ => {
                last_cmd = ' ';
            }
        }
    }
    segs
}

pub fn build_geometry(
    factory: &ID2D1Factory1,
    segs: &[Seg],
    fill: bool,
) -> windows::core::Result<ID2D1PathGeometry1> {
    unsafe {
        let geom = factory.CreatePathGeometry()?;
        let sink = geom.Open()?;
        sink.SetFillMode(D2D1_FILL_MODE_WINDING);

        let mut in_figure = false;

        for seg in segs {
            match *seg {
                Seg::MoveTo(x, y) => {
                    if in_figure {
                        sink.EndFigure(D2D1_FIGURE_END_OPEN);
                    }
                    sink.BeginFigure(
                        Vector2 { X: x, Y: y },
                        if fill {
                            D2D1_FIGURE_BEGIN_FILLED
                        } else {
                            D2D1_FIGURE_BEGIN_HOLLOW
                        },
                    );
                    in_figure = true;
                }
                Seg::LineTo(x, y) => {
                    if in_figure {
                        sink.AddLine(Vector2 { X: x, Y: y });
                    }
                }
                Seg::BezierTo(x1, y1, x2, y2, x, y) => {
                    if in_figure {
                        let bez = D2D1_BEZIER_SEGMENT {
                            point1: Vector2 { X: x1, Y: y1 },
                            point2: Vector2 { X: x2, Y: y2 },
                            point3: Vector2 { X: x, Y: y },
                        };
                        sink.AddBezier(&bez);
                    }
                }
                Seg::QuadTo(x1, y1, x, y) => {
                    if in_figure {
                        let quad = D2D1_QUADRATIC_BEZIER_SEGMENT {
                            point1: Vector2 { X: x1, Y: y1 },
                            point2: Vector2 { X: x, Y: y },
                        };
                        sink.AddQuadraticBezier(&quad);
                    }
                }
                Seg::ArcTo {
                    rx,
                    ry,
                    rot,
                    large,
                    sweep,
                    x,
                    y,
                } => {
                    if in_figure {
                        let arc = D2D1_ARC_SEGMENT {
                            point: Vector2 { X: x, Y: y },
                            size: D2D_SIZE_F {
                                width: rx,
                                height: ry,
                            },
                            rotationAngle: rot,
                            sweepDirection: if sweep {
                                D2D1_SWEEP_DIRECTION_CLOCKWISE
                            } else {
                                D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE
                            },
                            arcSize: if large {
                                D2D1_ARC_SIZE_LARGE
                            } else {
                                D2D1_ARC_SIZE_SMALL
                            },
                        };
                        sink.AddArc(&arc);
                    }
                }
                Seg::Close => {
                    if in_figure {
                        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                        in_figure = false;
                    }
                }
            }
        }

        if in_figure {
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
        }
        sink.Close()?;
        Ok(geom)
    }
}

pub fn circle(cx: f32, cy: f32, r: f32) -> Vec<Seg> {
    vec![
        Seg::MoveTo(cx - r, cy),
        Seg::ArcTo {
            rx: r,
            ry: r,
            rot: 0.0,
            large: false,
            sweep: true,
            x: cx + r,
            y: cy,
        },
        Seg::ArcTo {
            rx: r,
            ry: r,
            rot: 0.0,
            large: false,
            sweep: true,
            x: cx - r,
            y: cy,
        },
        Seg::Close,
    ]
}

pub fn rounded_rect(x: f32, y: f32, w: f32, h: f32, rx: f32) -> Vec<Seg> {
    let rx = rx.min(w / 2.0).min(h / 2.0);
    if rx <= 0.0 {
        return vec![
            Seg::MoveTo(x, y),
            Seg::LineTo(x + w, y),
            Seg::LineTo(x + w, y + h),
            Seg::LineTo(x, y + h),
            Seg::Close,
        ];
    }
    vec![
        Seg::MoveTo(x + rx, y),
        Seg::LineTo(x + w - rx, y),
        Seg::ArcTo {
            rx,
            ry: rx,
            rot: 0.0,
            large: false,
            sweep: true,
            x: x + w,
            y: y + rx,
        },
        Seg::LineTo(x + w, y + h - rx),
        Seg::ArcTo {
            rx,
            ry: rx,
            rot: 0.0,
            large: false,
            sweep: true,
            x: x + w - rx,
            y: y + h,
        },
        Seg::LineTo(x + rx, y + h),
        Seg::ArcTo {
            rx,
            ry: rx,
            rot: 0.0,
            large: false,
            sweep: true,
            x,
            y: y + h - rx,
        },
        Seg::LineTo(x, y + rx),
        Seg::ArcTo {
            rx,
            ry: rx,
            rot: 0.0,
            large: false,
            sweep: true,
            x: x + rx,
            y,
        },
        Seg::Close,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_line() {
        let s = parse_svg_path("M10 20 L30 40 Z");
        assert_eq!(
            s,
            vec![Seg::MoveTo(10.0, 20.0), Seg::LineTo(30.0, 40.0), Seg::Close]
        );
    }

    #[test]
    fn parse_arc_and_negative() {
        let s = parse_svg_path("M4 14a2 2 0 0 0 2 2h1v-4H6a2 2 0 0 0-2 2z");
        assert!(!s.is_empty());
        assert_eq!(s[0], Seg::MoveTo(4.0, 14.0));
        assert!(matches!(s[1], Seg::ArcTo { rx, ry, large: false, sweep: false, .. } if (rx - 2.0).abs() < 1e-4 && (ry - 2.0).abs() < 1e-4));
    }
}
