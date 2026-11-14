//! qr-probe <sample-id> [variant-width]: run vela-qr's stages on one corpus image,
//! and compare its binarization and finder patterns with rxing's.
use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../out/corpus");
    let manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    let s = manifest["samples"].as_array().unwrap().iter().find(|s| s["id"] == args[1].as_str()).expect("sample");
    let (file, w, h) = match args.get(2).filter(|a| a.as_str() != "-") {
        None => (s["file"].as_str().unwrap().to_owned(), s["width"].as_u64().unwrap(), s["height"].as_u64().unwrap()),
        Some(width) => {
            let v = s["ladder"].as_array().unwrap().iter().chain([&s["camera"]]).find(|v| v["w"].to_string() == *width).expect("variant");
            (v["file"].as_str().unwrap().to_owned(), v["w"].as_u64().unwrap(), v["h"].as_u64().unwrap())
        }
    };
    let mut luma = Vec::new();
    flate2::read::GzDecoder::new(std::fs::File::open(dir.join(file)).unwrap()).read_to_end(&mut luma).unwrap();
    let (w, h) = (w as usize, h as usize);
    println!("{} {w}x{h} expected {}", args[1], s["expected"]);
    print!("{}", vela_qr::probe(&luma, w, h));

    // rxing, same image.
    use rxing::Binarizer;
    let src = rxing::Luma8Source::new_with_slice(&luma, w as u32, h as u32).unwrap();
    let hb = rxing::common::HybridBinarizer::new(src);
    let m = hb.get_black_matrix().unwrap();
    let mine = vela_qr::probe_binarize(&luma, w, h);
    let diff = (0..w * h).filter(|&i| m.get((i % w) as u32, (i / w) as u32) != (mine[i] != 0)).count();
    println!("binarization differs in {diff} pixels");
    let fps = rxing::qrcode::cpp_port::detector::FindFinderPatterns(m, true, 0);
    println!("rxing finder patterns: {}", fps.len());
    for c in &fps {
        println!("  rx fp ({:.1},{:.1}) size {}", c.p.x, c.p.y, c.size);
    }
    if let Some(y) = args.get(3) {
        let y: usize = y.parse().unwrap();
        print!("{}", vela_qr::probe_row(&luma, w, h, y, 240, 340));
    }
    let ours = vela_qr::probe_finders(&luma, w, h);
    println!("vela finder patterns: {}", ours.len());
}
