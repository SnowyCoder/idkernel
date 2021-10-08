use core::fmt;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct MemorySize(pub usize);

impl fmt::Display for MemorySize {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        const SUFFIXES: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
        let mut i = 0;
        let mut x = self.0;

        while x >= 1024 && i < SUFFIXES.len() - 1 {
            x /= 1024;
            i += 1;
        }
        write!(f, "{}{}", x, SUFFIXES[i])
    }
}
