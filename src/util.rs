use paste::paste;

use crate::muldiv::MulDiv;
use crate::typenum::{NInt, NonZero, Unsigned, U10};
use crate::Fix;

/// Domain specific extensions to the `Fix` type as it's used in this project.
pub trait FixExt: Sized {
    /// This precision's equivalent of 1.
    const ONE: Self;
}

macro_rules! impl_fix_ext {
    ($bits:ident) => {
        paste! {
            impl<U> FixExt for Fix<$bits, U10, NInt<U>>
            where
                U: Unsigned + NonZero,
            {
                const ONE: Self =
                    Fix::constant((10 as $bits).pow(U::U32));
            }
        }
    };
}

impl_fix_ext!(u8);
impl_fix_ext!(u16);
impl_fix_ext!(u32);
impl_fix_ext!(u64);
impl_fix_ext!(u128);
impl_fix_ext!(usize);
impl_fix_ext!(i8);
impl_fix_ext!(i16);
impl_fix_ext!(i32);
impl_fix_ext!(i64);
impl_fix_ext!(i128);
impl_fix_ext!(isize);

impl<Bits, Base, Exp> Fix<Bits, Base, Exp>
where
    Self: FixExt,
{
    /// This precision's equivalent of 1.
    #[must_use]
    pub const fn one() -> Self {
        <Self as FixExt>::ONE
    }
}

impl<Bits, Exp> Fix<Bits, U10, Exp>
where
    Self: FixExt,
    Bits: MulDiv<Output = Bits>,
{
    /// Converts to another _Exp_, returning `None` on overflow.
    ///
    /// ```
    /// use fix::prelude::*;
    /// let source = UFix64::<N3>::new(5u64);
    /// let target = source.checked_convert::<N6>();
    /// assert_eq!(target, Some(UFix64::<N6>::new(5_000u64)));
    /// ```
    pub fn checked_convert<ToExp>(self) -> Option<Fix<Bits, U10, ToExp>>
    where
        Fix<Bits, U10, ToExp>: FixExt,
    {
        let target_one = Fix::<Bits, U10, ToExp>::one();
        let source_one = Self::one();
        target_one.mul_div_floor(self, source_one)
    }

    /// Converts to another _Exp_ rounding up, returning `None` on overflow.
    ///
    /// ```
    /// use fix::prelude::*;
    /// let source = UFix64::<N6>::new(5_001u64);
    /// let target = source.checked_convert_ceil::<N3>();
    /// assert_eq!(target, Some(UFix64::<N3>::new(6u64)));
    /// ```
    pub fn checked_convert_ceil<ToExp>(self) -> Option<Fix<Bits, U10, ToExp>>
    where
        Fix<Bits, U10, ToExp>: FixExt,
    {
        let target_one = Fix::<Bits, U10, ToExp>::one();
        let source_one = Self::one();
        target_one.mul_div_ceil(self, source_one)
    }
}
