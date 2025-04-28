#[inline]
pub fn escape_minecraft(inp: &str) -> String {
    inp.replace("\u{00a7}", "&")
}
