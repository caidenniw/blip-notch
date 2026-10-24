//! Vector icon definitions and D2D drawing.
use crate::gfx::svg::{build_geometry, circle, parse_svg_path, rounded_rect};
use windows::Win32::Graphics::Direct2D::*;
use windows_numerics::Matrix3x2;

#[derive(Clone, Copy, Debug)]
pub enum PartKind {
    Path(&'static str),
    Circle(f32, f32, f32),
    Rect(f32, f32, f32, f32, f32),
}

#[derive(Clone, Copy, Debug)]
pub struct IconPart {
    pub kind: PartKind,
    pub fill: bool,
    pub stroke: Option<f32>,
    pub round_cap: bool,
    pub round_join: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Icon {
    pub view: f32,
    pub parts: &'static [IconPart],
}

pub fn round_stroke_style(factory: &ID2D1Factory1) -> windows::core::Result<ID2D1StrokeStyle1> {
    let props = D2D1_STROKE_STYLE_PROPERTIES1 {
        startCap: D2D1_CAP_STYLE_ROUND,
        endCap: D2D1_CAP_STYLE_ROUND,
        dashCap: D2D1_CAP_STYLE_ROUND,
        lineJoin: D2D1_LINE_JOIN_ROUND,
        miterLimit: 1.0,
        dashStyle: D2D1_DASH_STYLE_SOLID,
        dashOffset: 0.0,
        transformType: D2D1_STROKE_TRANSFORM_TYPE_NORMAL,
    };
    unsafe { factory.CreateStrokeStyle(&props, None) }
}

pub fn draw(
    dc: &ID2D1DeviceContext,
    factory: &ID2D1Factory1,
    icon: &Icon,
    x: f32,
    y: f32,
    size: f32,
    brush: &ID2D1Brush,
    round_style: Option<&ID2D1StrokeStyle1>,
) -> windows::core::Result<()> {
    let scale = size / icon.view;
    unsafe {
        let mut old_xf = Matrix3x2::identity();
        dc.GetTransform(&mut old_xf);
        let xf = old_xf * Matrix3x2::translation(x, y) * Matrix3x2::scale(scale, scale);
        dc.SetTransform(&xf);

        for part in icon.parts {
            let segs = match part.kind {
                PartKind::Path(d) => parse_svg_path(d),
                PartKind::Circle(cx, cy, r) => circle(cx, cy, r),
                PartKind::Rect(rx, ry, w, h, rad) => rounded_rect(rx, ry, w, h, rad),
            };
            if segs.is_empty() {
                continue;
            }
            if part.fill {
                if let Ok(geom) = build_geometry(factory, &segs, true) {
                    dc.FillGeometry(&geom, brush, None);
                }
            }
            if let Some(w) = part.stroke {
                if let Ok(geom) = build_geometry(factory, &segs, false) {
                    let st: Option<&ID2D1StrokeStyle> = if part.round_cap || part.round_join {
                        round_style.map(|s| &**s)
                    } else {
                        None
                    };
                    dc.DrawGeometry(&geom, brush, w, st);
                }
            }
        }
        dc.SetTransform(&old_xf);
    }
    Ok(())
}

// ==================== NAV ICONS ====================

pub const NAV_TODAY: Icon = Icon {
    view: 24.0,
    parts: &[
        IconPart {
            kind: PartKind::Circle(12.0, 12.0, 4.0),
            fill: false,
            stroke: Some(1.8),
            round_cap: true,
            round_join: true,
        },
        IconPart {
            kind: PartKind::Path(
                "M12 2.5v2.5M12 19v2.5M2.5 12H5M19 12h2.5M5.3 5.3l1.8 1.8M16.9 16.9l1.8 1.8M5.3 18.7l1.8-1.8M16.9 7.1l1.8-1.8",
            ),
            fill: false,
            stroke: Some(1.8),
            round_cap: true,
            round_join: true,
        },
    ],
};

pub const NAV_MUSIC: Icon = Icon {
    view: 24.0,
    parts: &[
        IconPart {
            kind: PartKind::Path("M9 18V5l10-2v13"),
            fill: false,
            stroke: Some(1.8),
            round_cap: true,
            round_join: true,
        },
        IconPart {
            kind: PartKind::Circle(6.0, 18.0, 3.0),
            fill: false,
            stroke: Some(1.8),
            round_cap: true,
            round_join: true,
        },
        IconPart {
            kind: PartKind::Circle(16.0, 16.0, 3.0),
            fill: false,
            stroke: Some(1.8),
            round_cap: true,
            round_join: true,
        },
    ],
};

pub const NAV_FOCUS: Icon = Icon {
    view: 24.0,
    parts: &[
        IconPart {
            kind: PartKind::Circle(12.0, 12.0, 8.0),
            fill: false,
            stroke: Some(1.8),
            round_cap: true,
            round_join: true,
        },
        IconPart {
            kind: PartKind::Path("M12 7v5l3 2"),
            fill: false,
            stroke: Some(1.8),
            round_cap: true,
            round_join: true,
        },
    ],
};

pub const NAV_TODO: Icon = Icon {
    view: 24.0,
    parts: &[
        IconPart {
            kind: PartKind::Rect(4.0, 4.0, 16.0, 16.0, 3.0),
            fill: false,
            stroke: Some(1.8),
            round_cap: true,
            round_join: true,
        },
        IconPart {
            kind: PartKind::Path("M8 9h8M8 12h5"),
            fill: false,
            stroke: Some(1.8),
            round_cap: true,
            round_join: true,
        },
    ],
};

pub const NAV_WATER: Icon = Icon {
    view: 24.0,
    parts: &[IconPart {
        kind: PartKind::Path("M12 3s6 6.1 6 11a6 6 0 0 1-12 0c0-4.9 6-11 6-11z"),
        fill: false,
        stroke: Some(1.8),
        round_cap: true,
        round_join: true,
    }],
};

pub const NAV_SHELF: Icon = Icon {
    view: 24.0,
    parts: &[IconPart {
        kind: PartKind::Path("M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"),
        fill: false,
        stroke: Some(1.8),
        round_cap: true,
        round_join: true,
    }],
};

pub const NAV_CHAT: Icon = Icon {
    view: 24.0,
    parts: &[IconPart {
        kind: PartKind::Path("M4 5h16v11H9l-5 4z"),
        fill: false,
        stroke: Some(1.8),
        round_cap: true,
        round_join: true,
    }],
};

// ==================== MEDIA CTL ====================

pub const CTL_PREV: Icon = Icon {
    view: 24.0,
    parts: &[IconPart {
        kind: PartKind::Path("M11 6 3 12l8 6zM21 6l-8 6 8 6z"),
        fill: true,
        stroke: None,
        round_cap: false,
        round_join: false,
    }],
};

pub const CTL_NEXT: Icon = Icon {
    view: 24.0,
    parts: &[IconPart {
        kind: PartKind::Path("m13 6 8 6-8 6zM3 6l8 6-8 6z"),
        fill: true,
        stroke: None,
        round_cap: false,
        round_join: false,
    }],
};

pub const CTL_PLAY: Icon = Icon {
    view: 24.0,
    parts: &[IconPart {
        kind: PartKind::Path("M8 5v14l11-7z"),
        fill: true,
        stroke: None,
        round_cap: false,
        round_join: false,
    }],
};

pub const CTL_PAUSE: Icon = Icon {
    view: 24.0,
    parts: &[
        IconPart {
            kind: PartKind::Path("M7 5h4v14H7z"),
            fill: true,
            stroke: None,
            round_cap: false,
            round_join: false,
        },
        IconPart {
            kind: PartKind::Path("M13 5h4v14h-4z"),
            fill: true,
            stroke: None,
            round_cap: false,
            round_join: false,
        },
    ],
};

pub const CTL_LYRICS: Icon = Icon {
    view: 24.0,
    parts: &[
        IconPart {
            kind: PartKind::Path("M4 5h16v11H9l-5 4z"),
            fill: false,
            stroke: Some(2.0),
            round_cap: false,
            round_join: true,
        },
        IconPart {
            kind: PartKind::Path("M8 9h8M8 12h5"),
            fill: false,
            stroke: Some(1.6),
            round_cap: true,
            round_join: false,
        },
    ],
};

// ==================== COMMON ACTIONS ====================

pub const PIN: Icon = Icon {
    view: 24.0,
    parts: &[IconPart {
        kind: PartKind::Path("M16 12V4H8v8l-2 2v2h5v6l1 1 1-1v-6h5v-2z"),
        fill: false,
        stroke: Some(1.8),
        round_cap: false,
        round_join: true,
    }],
};

pub const GEAR: Icon = Icon {
    view: 24.0,
    parts: &[
        IconPart {
            kind: PartKind::Circle(12.0, 12.0, 3.0),
            fill: false,
            stroke: Some(2.0),
            round_cap: true,
            round_join: true,
        },
        IconPart {
            kind: PartKind::Path(
                "M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M18.4 5.6l-2.1 2.1M7.7 16.3l-2.1 2.1",
            ),
            fill: false,
            stroke: Some(1.6),
            round_cap: true,
            round_join: true,
        },
    ],
};

pub const SEND: Icon = Icon {
    view: 24.0,
    parts: &[IconPart {
        kind: PartKind::Path("M5 12h14M13 6l6 6-6 6"),
        fill: false,
        stroke: Some(2.0),
        round_cap: true,
        round_join: true,
    }],
};

pub const MIC: Icon = Icon {
    view: 24.0,
    parts: &[
        IconPart {
            kind: PartKind::Rect(9.0, 3.0, 6.0, 12.0, 3.0),
            fill: false,
            stroke: Some(2.0),
            round_cap: true,
            round_join: true,
        },
        IconPart {
            kind: PartKind::Path("M5 11a7 7 0 0 0 14 0M12 18v3"),
            fill: false,
            stroke: Some(2.0),
            round_cap: true,
            round_join: true,
        },
    ],
};

pub const CHECK: Icon = Icon {
    view: 24.0,
    parts: &[IconPart {
        kind: PartKind::Path("m5 12 4 4L19 6"),
        fill: false,
        stroke: Some(2.0),
        round_cap: true,
        round_join: true,
    }],
};

pub const CLOSE: Icon = Icon {
    view: 24.0,
    parts: &[IconPart {
        kind: PartKind::Path("m15 9 6 6M21 9l-6 6"),
        fill: false,
        stroke: Some(1.8),
        round_cap: true,
        round_join: true,
    }],
};

// ==================== HUD ICONS ====================

pub const HUD_SPEAKER: Icon = Icon {
    view: 24.0,
    parts: &[
        IconPart {
            kind: PartKind::Path("M4 9h3l4-4v14l-4-4H4z"),
            fill: true,
            stroke: None,
            round_cap: false,
            round_join: false,
        },
        IconPart {
            kind: PartKind::Path("M15.5 8.5a5 5 0 0 1 0 7M18 6a8 8 0 0 1 0 12"),
            fill: false,
            stroke: Some(1.7),
            round_cap: true,
            round_join: true,
        },
    ],
};

pub const HUD_MUTED: Icon = Icon {
    view: 24.0,
    parts: &[
        IconPart {
            kind: PartKind::Path("M4 9h3l4-4v14l-4-4H4z"),
            fill: true,
            stroke: None,
            round_cap: false,
            round_join: false,
        },
        IconPart {
            kind: PartKind::Path("m15 9 6 6M21 9l-6 6"),
            fill: false,
            stroke: Some(1.8),
            round_cap: true,
            round_join: true,
        },
    ],
};

pub const HUD_CAPS: Icon = Icon {
    view: 24.0,
    parts: &[
        IconPart {
            kind: PartKind::Path("M12 4 4 12h5v4h6v-4h5z"),
            fill: true,
            stroke: None,
            round_cap: false,
            round_join: false,
        },
        IconPart {
            kind: PartKind::Rect(7.0, 18.0, 10.0, 2.2, 1.1),
            fill: true,
            stroke: None,
            round_cap: false,
            round_join: false,
        },
    ],
};

pub const HUD_WIFI: Icon = Icon {
    view: 24.0,
    parts: &[IconPart {
        kind: PartKind::Path("M12 18h.01M5 11a10 10 0 0 1 14 0M8.5 14.5a5 5 0 0 1 7 0"),
        fill: false,
        stroke: Some(2.0),
        round_cap: true,
        round_join: true,
    }],
};

pub const HUD_SUN: Icon = Icon {
    view: 24.0,
    parts: &[
        IconPart {
            kind: PartKind::Circle(12.0, 12.0, 4.0),
            fill: false,
            stroke: Some(2.0),
            round_cap: true,
            round_join: true,
        },
        IconPart {
            kind: PartKind::Path(
                "M12 2.5v2.5M12 19v2.5M2.5 12H5M19 12h2.5M5.3 5.3l1.8 1.8M16.9 16.9l1.8 1.8M5.3 18.7l1.8-1.8M16.9 7.1l1.8-1.8",
            ),
            fill: false,
            stroke: Some(1.8),
            round_cap: true,
            round_join: true,
        },
    ],
};

pub const HUD_BT: Icon = Icon {
    view: 24.0,
    parts: &[IconPart {
        kind: PartKind::Path("m7 7 10 10-5 4V3l5 4L7 17"),
        fill: false,
        stroke: Some(2.0),
        round_cap: true,
        round_join: true,
    }],
};

pub const HUD_HEADPHONE: Icon = Icon {
    view: 24.0,
    parts: &[IconPart {
        kind: PartKind::Path("M4 14v-2a8 8 0 0 1 16 0v2M4 14a2 2 0 0 0 2 2h1v-4H6a2 2 0 0 0-2 2zM20 14a2 2 0 0 1-2 2h-1v-4h1a2 2 0 0 1 2 2z"),
        fill: false,
        stroke: Some(1.8),
        round_cap: true,
        round_join: true,
    }],
};

pub const HUD_BATTERY: Icon = Icon {
    view: 24.0,
    parts: &[
        IconPart {
            kind: PartKind::Rect(4.0, 7.0, 14.0, 10.0, 2.0),
            fill: false,
            stroke: Some(1.8),
            round_cap: true,
            round_join: true,
        },
        IconPart {
            kind: PartKind::Path("M19 10v4"),
            fill: false,
            stroke: Some(2.0),
            round_cap: true,
            round_join: true,
        },
    ],
};

// ==================== SHELF FILE KIND ICONS ====================

pub const FILE_FOLDER: Icon = NAV_SHELF;
pub const FILE_AUDIO: Icon = NAV_MUSIC;

pub const FILE_IMAGE: Icon = Icon {
    view: 24.0,
    parts: &[
        IconPart {
            kind: PartKind::Rect(3.0, 5.0, 18.0, 14.0, 2.0),
            fill: false,
            stroke: Some(1.8),
            round_cap: true,
            round_join: true,
        },
        IconPart {
            kind: PartKind::Circle(8.5, 10.0, 1.4),
            fill: true,
            stroke: None,
            round_cap: false,
            round_join: false,
        },
        IconPart {
            kind: PartKind::Path("m4 17 5-5 4 4 3-2 4 4"),
            fill: false,
            stroke: Some(1.6),
            round_cap: true,
            round_join: true,
        },
    ],
};

pub const FILE_CODE: Icon = Icon {
    view: 24.0,
    parts: &[IconPart {
        kind: PartKind::Path("m9 7-5 5 5 5M15 7l5 5-5 5M13 4l-2 16"),
        fill: false,
        stroke: Some(1.8),
        round_cap: true,
        round_join: true,
    }],
};

pub const FILE_GENERIC: Icon = Icon {
    view: 24.0,
    parts: &[
        IconPart {
            kind: PartKind::Path("M6 2h8l4 4v16H6z"),
            fill: false,
            stroke: Some(1.8),
            round_cap: true,
            round_join: true,
        },
        IconPart {
            kind: PartKind::Path("M14 2v5h5M8 12h8M8 16h8"),
            fill: false,
            stroke: Some(1.6),
            round_cap: true,
            round_join: true,
        },
    ],
};
