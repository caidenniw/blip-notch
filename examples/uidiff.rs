//! `cargo run --example uidiff -- target/ref`: compare `<name>.edge.png` with `<name>.native.png`.
use std::{fs::File, io::BufWriter, path::Path, process::exit};

const LIMIT: f64 = 1.5;

/// Decode to straight-alpha RGBA8.
fn load(p: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    let mut dec = png::Decoder::new(File::open(p).map_err(|e| e.to_string())?);
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut r = dec.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; r.output_buffer_size()];
    let info = r.next_frame(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf.chunks(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("indexed png not expanded".into()),
    };
    Ok((info.width, info.height, rgba))
}

/// Straight alpha over #808080.
fn over_grey(px: &[u8]) -> [i32; 3] {
    let a = px[3] as i32;
    let f = |c: u8| (c as i32 * a + 128 * (255 - a) + 127) / 255;
    [f(px[0]), f(px[1]), f(px[2])]
}

fn compare(dir: &Path, name: &str) -> Result<f64, String> {
    let (ew, eh, e) = load(&dir.join(format!("{name}.edge.png")))?;
    let (nw, nh, n) = load(&dir.join(format!("{name}.native.png")))?;
    if (ew, eh) != (nw, nh) {
        eprintln!("{name}: size {ew}x{eh} vs {nw}x{nh}");
        return Ok(100.0);
    }
    let mut out = Vec::with_capacity((ew * eh * 3) as usize);
    let mut bad = 0usize;
    for (pe, pn) in e.chunks(4).zip(n.chunks(4)) {
        let (a, b) = (over_grey(pe), over_grey(pn));
        if (0..3).any(|i| (a[i] - b[i]).abs() > 24) {
            bad += 1;
            out.extend_from_slice(&[255, 0, 0]);
        } else {
            out.extend(a.iter().map(|&c| (c * 3 / 10) as u8));
        }
    }
    let f = File::create(dir.join(format!("{name}.diff.png"))).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(BufWriter::new(f), ew, eh);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().and_then(|mut w| w.write_image_data(&out)).map_err(|e| e.to_string())?;
    Ok(bad as f64 * 100.0 / (ew as f64 * eh as f64))
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "target/ref".into());
    let dir = Path::new(&dir);
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| { eprintln!("{}: {e}", dir.display()); exit(2) })
        .filter_map(|e| e.ok()?.file_name().into_string().ok()?.strip_suffix(".edge.png").map(String::from))
        .filter(|n| dir.join(format!("{n}.native.png")).exists())
        .collect();
    names.sort();
    let mut fail = false;
    for name in &names {
        let pct = compare(dir, name).unwrap_or_else(|e| { eprintln!("{name}: {e}"); 100.0 });
        println!("{name}: {pct:.2}% differ");
        fail |= pct > LIMIT;
    }
    if names.is_empty() { eprintln!("no <name>.edge.png + <name>.native.png pairs in {}", dir.display()); }
    exit(fail as i32);
}
