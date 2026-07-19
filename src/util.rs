use paste::paste;
#[cfg(feature = "typed-floats")]
use typed_floats::StrictlyPositiveFinite;

use crate::muldiv::MulDiv;
use crate::typenum::{Integer, NInt, NonZero, Unsigned, U10};
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

macro_rules! impl_to_f64 {
    ($bits:ident) => {
        impl<Exp: Integer> Fix<$bits, U10, Exp> {
            /// Approximate `f64` value of this fixed-point number.
            ///
            /// Precision loss above 2^53 bits; intended for offchain
            /// analytics, never for onchain math.
            ///
            /// ```
            /// use fix::prelude::*;
            /// let x = UFix64::<N6>::new(1_500_000u64);
            /// assert!((x.to_f64() - 1.5).abs() < f64::EPSILON);
            /// ```
            #[must_use]
            #[allow(clippy::cast_precision_loss)]
            pub fn to_f64(self) -> f64 {
                self.bits as f64 * 10f64.powi(Exp::to_i32())
            }
        }
    };
}

impl_to_f64!(u64);
impl_to_f64!(i64);

#[cfg(feature = "typed-floats")]
impl<Exp: Integer> Fix<u64, U10, Exp> {
    /// Strictly positive finite `f64` view; `None` when zero.
    #[must_use]
    pub fn to_positive_f64(self) -> Option<StrictlyPositiveFinite> {
        StrictlyPositiveFinite::try_from(self.to_f64()).ok()
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

#[cfg(test)]
mod tests {
    use crate::aliases::decimal::{IFix64, UFix64};
    #[cfg(feature = "typed-floats")]
    use crate::typenum::N6;
    use crate::typenum::{N3, N9};

    #[test]
    fn to_f64_small_bits_exact() {
        let x = UFix64::<N3>::new(1_500u64);
        assert!((x.to_f64() - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn to_f64_negative_bits_and_exp() {
        let x = IFix64::<N9>::new(-975i64);
        assert!((x.to_f64() - -9.75e-7).abs() < 1e-21);
    }

    #[test]
    #[allow(clippy::excessive_precision)]
    fn to_f64_max_bits_relative_error() {
        let got = UFix64::<N9>::new(u64::MAX).to_f64();
        let expected = 18_446_744_073.709_551_615_f64;
        assert!(((got - expected) / expected).abs() < 1e-15);
    }

    #[cfg(feature = "typed-floats")]
    #[test]
    fn to_positive_f64_zero_is_none() {
        assert!(UFix64::<N6>::zero().to_positive_f64().is_none());
    }

    #[cfg(feature = "typed-floats")]
    #[test]
    fn to_positive_f64_nonzero_is_some() {
        let x = UFix64::<N6>::new(2_500_000u64);
        let positive = x.to_positive_f64().map(f64::from);
        assert_eq!(positive, Some(x.to_f64()));
    }
}
