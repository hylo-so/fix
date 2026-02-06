use crate::muldiv::MulDiv;
use crate::typenum::consts::Z0;
use crate::typenum::{Integer, IsLess, B1, U10};
use crate::{Fix, FromUnsigned, Pow};

/// Domain specific extensions to the `Fix` type as it's used in this project.
pub trait FixExt: Sized {
    /// This precision's equivalent of 1.
    fn one() -> Self;
}

impl<Bits, Exp> FixExt for Fix<Bits, U10, Exp>
where
    Bits: FromUnsigned + Pow,
    Exp: Integer + IsLess<Z0, Output = B1>,
{
    fn one() -> Self {
        let base = Bits::from_unsigned::<U10>();
        Fix::new(base.pow(Exp::to_i32().unsigned_abs()))
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
}
