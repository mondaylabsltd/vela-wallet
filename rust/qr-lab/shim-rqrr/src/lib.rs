//! rqrr (a Rust port of quirc): first grid that decodes wins.
fn decode(luma: &[u8], w: u32, h: u32) -> Option<String> {
    let (w, h) = (w as usize, h as usize);
    let mut img = rqrr::PreparedImage::prepare_from_greyscale(w, h, |x, y| luma[y * w + x]);
    img.detect_grids()
        .into_iter()
        .find_map(|grid| grid.decode().ok().map(|(_, text)| text))
}

lab_abi::export_decoder!(decode);
