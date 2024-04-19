use crate::Fix;
use anchor_lang::prelude::Space;

macro_rules! fix_init_space {
    ($ty:ident) => {
        impl<Base, Exp> Space for Fix<$ty, Base, Exp> {
            const INIT_SPACE: usize = core::mem::size_of::<$ty>();
        }
    };
}

fix_init_space!(u8);
fix_init_space!(u16);
fix_init_space!(u32);
fix_init_space!(u64);
fix_init_space!(u128);
