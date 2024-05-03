use crate::Fix;
use anchor_lang::prelude::{borsh, AnchorDeserialize, AnchorSerialize, Space};

impl<Bits, Base, Exp> AnchorSerialize for Fix<Bits, Base, Exp>
where
    Bits: AnchorSerialize,
{
    fn serialize<W>(&self, w: &mut W) -> Result<(), borsh::maybestd::io::Error>
    where
        W: borsh::maybestd::io::Write,
    {
        self.bits.serialize(w)
    }
}

impl<Bits, Base, Exp> AnchorDeserialize for Fix<Bits, Base, Exp>
where
    Bits: AnchorDeserialize,
{
    fn deserialize_reader<R>(r: &mut R) -> Result<Self, borsh::maybestd::io::Error>
    where
        R: borsh::maybestd::io::Read,
    {
        AnchorDeserialize::deserialize_reader(r).map(Fix::<Bits, Base, Exp>::new)
    }
}

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
fix_init_space!(usize);
fix_init_space!(i8);
fix_init_space!(i16);
fix_init_space!(i32);
fix_init_space!(i64);
fix_init_space!(i128);
fix_init_space!(isize);
