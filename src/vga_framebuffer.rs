use bootloader::boot_info::{FrameBuffer, PixelFormat};
use font8x8::BASIC_UNICODE;
use alloc::fmt;
use conquer_once::spin::OnceCell;
use spin::Mutex;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Color(pub [u8; 3]);

pub struct VgaColorWriter {
    curr_x: usize,
    curr_y: usize,
    max_x: usize,
    max_y: usize,
    buffer: &'static mut FrameBuffer,
    pub background: Color,
    pub foreground: Color,
}

impl VgaColorWriter {
    pub fn new(buffer: &'static mut FrameBuffer) -> Self {
        let max_x = buffer.info().horizontal_resolution / 8;
        let max_y = buffer.info().vertical_resolution / 8;
        VgaColorWriter {
            buffer,
            curr_x: 0,
            curr_y: 0,
            max_x,
            max_y,
            background: Color([0, 0, 0]),
            foreground: Color([255, 255, 255]),
        }
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: Color) {
        let index = (y * self.buffer.info().stride + x) * 8 * self.buffer.info().bytes_per_pixel;
        let pixels = match self.buffer.info().pixel_format {
            PixelFormat::RGB => 3,
            PixelFormat::BGR => 3,
            PixelFormat::U8 => 1,
            _ => panic!("Unrecognized format"),
        };
        let mut i = index;
        for _y1 in 0..(h*8) {
            for _x1 in 0..(w*8) {
                for z in 0..pixels {
                    self.buffer.buffer_mut()[i + z] = color.0[z];
                }
                i += self.buffer.info().bytes_per_pixel;
            }
            i += (self.buffer.info().stride - w * 8) * self.buffer.info().bytes_per_pixel;
        }
    }

    pub fn splat_char(&mut self, c: u8, x: usize, y: usize) {
        let begin_pixel = (y * self.buffer.info().stride + x) * self.buffer.info().bytes_per_pixel;

        let image = BASIC_UNICODE[c as usize];
        let mut i = 0;
        let pixels = match self.buffer.info().pixel_format {
            PixelFormat::RGB => 3,
            PixelFormat::BGR => 3,
            PixelFormat::U8 => 1,
            _ => panic!("Unrecognized format"),
        };

        let bg = self.background;
        let fg = self.foreground;
        for y in 0..8 {
            let mut splat_row = image.byte_array()[y];
            for _x in 0..8 {
                let bit = (splat_row & 1) != 0;
                for z in 0..pixels {
                    self.buffer.buffer_mut()[begin_pixel + i + z] = if bit { fg.0[z] } else { bg.0[z] };
                }
                i += self.buffer.info().bytes_per_pixel;
                splat_row = splat_row >> 1;
            }
            i += (self.buffer.info().stride - 8) * self.buffer.info().bytes_per_pixel;
        }
    }

    pub fn advance_char(&mut self) {
        self.curr_x += 1;
        if self.curr_x >= self.max_x {
            self.new_line();
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                self.splat_char(byte, self.curr_x * 8, self.curr_y * 8);
                self.advance_char()
            }
        }
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // printable ASCII byte or newline
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // not part of printable ASCII range
                _ => self.write_byte(b'?'),
            }
        }
    }

    fn new_line(&mut self) {
        self.curr_y += 1;
        self.curr_x = 0;

        if self.curr_y >= self.max_y {
            self.curr_y = 0;
        }

        self.clean_row();
    }

    pub fn clean_row(&mut self) {
        self.fill_rect(0, self.curr_y, self.max_x, 1, self.background);
    }

    pub fn clean_screen(&mut self) {
        self.fill_rect(0, 0, self.max_x, self.max_y, self.background);
        self.curr_x = 0;
        self.curr_y = 0;
    }
}

impl fmt::Write for VgaColorWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

static VGA_WRITER: OnceCell<Mutex<VgaColorWriter>> = OnceCell::uninit();

pub fn init_vga_framebuffer(framebuffer: &'static mut FrameBuffer) {
    VGA_WRITER.try_init_once(move || {
        let mut buffer = VgaColorWriter::new(framebuffer);
        buffer.clean_screen();
        buffer.write_string("VGA text initializated\n");
        Mutex::new(buffer)
    }).expect("VGA writer already initialized");
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_framebuffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        VGA_WRITER.get().expect("VGA NOT INITIALIZED")
            .lock().write_fmt(args).unwrap();
    });
}
