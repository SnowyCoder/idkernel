#![no_std]

pub use include_dir_macros::include_dir;

pub struct InitDir(pub &'static [(&'static str, InitDirEntry)]);

impl InitDir {
    pub fn entry(&self, path: &'static str) -> Option<&'static InitDirEntry> {
        self.0.iter()
            .find(|x| x.0 == path)
            .map(|x| &x.1)
    }
}

pub enum InitDirEntry {
    File(&'static [u8]),
    Folder(InitDir),
}

#[repr(C)] // guarantee 'bytes' comes after '_align'
pub struct AlignedAs<Align, Bytes: ?Sized> {
    pub _align: [Align; 0],
    pub bytes: Bytes,
}

#[macro_export]
macro_rules! include_bytes_align_as {
    ($align_ty:ty, $path:literal) => {
        {  // const block expression to encapsulate the static
            use ::include_dir::AlignedAs;

            // this assignment is made possible by CoerceUnsized
            static ALIGNED: &AlignedAs::<$align_ty, [u8]> = &AlignedAs {
                _align: [],
                bytes: *include_bytes!($path),
            };

            // Why?? because of this "bug" here:
            // https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=f68680a7e3f556e9a67c7d8439fe1932
            static ALIGNED_BYTES: &'static [u8] = &ALIGNED.bytes;

            ALIGNED_BYTES
        }
    };
}
