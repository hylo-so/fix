use paste::paste;
#[cfg(feature = "typed-floats")]
use typed_floats::StrictlyPositiveFinite;

use crate::muldiv::MulDiv;
use crate::num_traits::ConstZero;
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

    /// Divides by `rhs` at the same precision, rounding down.
    /// `None` on overflow or division by zero.
    ///
    /// ```
    /// use fix::prelude::*;
    /// let a = UFix64::<N3>::new(10_000u64);
    /// let b = UFix64::<N3>::new(3_000u64);
    /// assert_eq!(a.div_floor(b), Some(UFix64::<N3>::new(3_333u64)));
    /// ```
    pub fn div_floor(self, rhs: Self) -> Option<Self>
    where
        Bits: ConstZero + PartialEq,
    {
        if rhs == Self::zero() {
            None
        } else {
            self.mul_div_floor(Self::one(), rhs)
        }
    }

    /// Divides by `rhs` at the same precision, rounding up.
    /// `None` on overflow or division by zero.
    ///
    /// ```
    /// use fix::prelude::*;
    /// let a = UFix64::<N3>::new(10_000u64);
    /// let b = UFix64::<N3>::new(3_000u64);
    /// assert_eq!(a.div_ceil(b), Some(UFix64::<N3>::new(3_334u64)));
    /// ```
    pub fn div_ceil(self, rhs: Self) -> Option<Self>
    where
        Bits: ConstZero + PartialEq,
    {
        if rhs == Self::zero() {
            None
        } else {
            self.mul_div_ceil(Self::one(), rhs)
        }
    }

    /// Multiplies by `rhs` at the same precision, rounding down.
    /// `None` on overflow.
    ///
    /// ```
    /// use fix::prelude::*;
    /// let a = UFix64::<N3>::new(1_001u64);
    /// assert_eq!(a.mul_floor(a), Some(UFix64::<N3>::new(1_002u64)));
    /// ```
    pub fn mul_floor(self, rhs: Self) -> Option<Self> {
        self.mul_div_floor(rhs, Self::one())
    }

    /// Multiplies by `rhs` at the same precision, rounding up.
    /// `None` on overflow.
    ///
    /// ```
    /// use fix::prelude::*;
    /// let a = UFix64::<N3>::new(1_001u64);
    /// assert_eq!(a.mul_ceil(a), Some(UFix64::<N3>::new(1_003u64)));
    /// ```
    pub fn mul_ceil(self, rhs: Self) -> Option<Self> {
        self.mul_div_ceil(rhs, Self::one())
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

    #[test]
    fn div_floor_rounds_down() {
        let a = UFix64::<N3>::new(10_000u64);
        let b = UFix64::<N3>::new(3_000u64);
        assert_eq!(a.div_floor(b), Some(UFix64::<N3>::new(3_333u64)));
    }

    #[test]
    fn div_ceil_rounds_up() {
        let a = UFix64::<N3>::new(10_000u64);
        let b = UFix64::<N3>::new(3_000u64);
        assert_eq!(a.div_ceil(b), Some(UFix64::<N3>::new(3_334u64)));
    }

    #[test]
    fn div_exact_floor_eq_ceil() {
        let a = UFix64::<N3>::new(9_000u64);
        let b = UFix64::<N3>::new(3_000u64);
        let exact = Some(UFix64::<N3>::new(3_000u64));
        assert_eq!(a.div_floor(b), exact);
        assert_eq!(a.div_ceil(b), exact);
    }

    #[test]
    fn div_by_zero_is_none() {
        let a = UFix64::<N3>::new(10_000u64);
        assert_eq!(a.div_floor(UFix64::<N3>::zero()), None);
        assert_eq!(a.div_ceil(UFix64::<N3>::zero()), None);
    }

    #[test]
    fn div_negative_rounds_toward_neg_infinity() {
        let a = IFix64::<N3>::new(-10_000i64);
        let b = IFix64::<N3>::new(3_000i64);
        assert_eq!(a.div_floor(b), Some(IFix64::<N3>::new(-3_334i64)));
        assert_eq!(a.div_ceil(b), Some(IFix64::<N3>::new(-3_333i64)));
    }

    #[test]
    fn mul_floor_rounds_down() {
        let a = UFix64::<N3>::new(1_001u64);
        assert_eq!(a.mul_floor(a), Some(UFix64::<N3>::new(1_002u64)));
    }

    #[test]
    fn mul_floor_exact() {
        let a = UFix64::<N3>::new(2_000u64);
        let b = UFix64::<N3>::new(1_500u64);
        assert_eq!(a.mul_floor(b), Some(UFix64::<N3>::new(3_000u64)));
    }

    #[test]
    fn mul_floor_overflow_is_none() {
        let a = UFix64::<N3>::new(u64::MAX);
        assert_eq!(a.mul_floor(a), None);
    }

    #[test]
    fn mul_ceil_rounds_up() {
        let a = UFix64::<N3>::new(1_001u64);
        assert_eq!(a.mul_ceil(a), Some(UFix64::<N3>::new(1_003u64)));
    }

    #[test]
    fn mul_ceil_exact() {
        let a = UFix64::<N3>::new(2_000u64);
        let b = UFix64::<N3>::new(1_500u64);
        assert_eq!(a.mul_ceil(b), Some(UFix64::<N3>::new(3_000u64)));
    }

    #[test]
    fn mul_ceil_overflow_is_none() {
        let a = UFix64::<N3>::new(u64::MAX);
        assert_eq!(a.mul_ceil(a), None);
    }
}
