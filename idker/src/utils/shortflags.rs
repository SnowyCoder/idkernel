use core::fmt;

use x86_64::structures::paging::PageTableFlags;


pub struct ShortFlags<F>(pub F);

macro_rules! impl_display {
    ($tname:ty, {$($print:literal $name:ident;)+}) => {
        impl fmt::Display for ShortFlags<$tname> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let mut first = true;

                $(
                    if self.0.contains(<$tname>::$name) {
                        if first {
                            first = false;
                        } else {
                            write!(f, "|")?;
                        }
                        write!(f, $print)?;
                    }
                )+

                Ok(())
            }
        }
    };
}

impl_display!(PageTableFlags, {
    "P" PRESENT;
    "W" WRITABLE;
    "UA" USER_ACCESSIBLE;
    "WT" WRITE_THROUGH;
    "NC" NO_CACHE;
    "A" ACCESSED;
    "D" DIRTY;
    "H" HUGE_PAGE;
    "G" GLOBAL;
    "NE" NO_EXECUTE;
});
