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
    "F" PRESENT;
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

/*impl fmt::Display for ShortFlags<PageTableFlags> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use PageTableFlags as F;

        write_all! {
            "F" F::PRESENT;
            "W" F::WRITABLE;
            "UA" F::USER_ACCESSIBLE;
            "WT" F::WRITE_THROUGH;
            "NC" F::NO_CACHE;
            "A" F::ACCESSED;
            "D" F::DIRTY;
            "H" F::HUGE_PAGE;
            "G" F::GLOBAL;
            "NE" F::NO_EXECUTE;
        };
        Ok(())
    }
}*/