use typenum::NInt;

use crate::typenum::{IsLess, NonZero, Unsigned, U10, U20};
use crate::Fix;

/// Domain specific extensions to the `Fix` type as it's used in this project.
pub trait FixExt: Sized {
    /// This precision's equivalent of 1.
    fn one() -> Self;
    fn zero() -> Self;
}

impl<Bits, Exp> FixExt for Fix<Bits, U10, NInt<Exp>>
where
    Exp: Unsigned + NonZero + IsLess<U20>,
    Bits: From<u64>,
{
    fn one() -> Fix<Bits, U10, NInt<Exp>> {
        Fix::new(U10::to_u64().pow(Exp::to_u32()).into())
    }

    fn zero() -> Self {
        Fix::new(0.into())
    }
}
