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
