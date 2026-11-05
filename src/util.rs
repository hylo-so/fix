use paste::paste;
#[cfg(feature = "typed-floats")]
use typed_floats::StrictlyPositiveFinite;

use crate::kernel::{MulDivKernel, Round};
use crate::muldiv::MulDiv;
use crate::num_traits::{float::FloatCore, ConstZero};
use crate::transcendental::{self, Scale};
use crate::typenum::{Integer, NInt, NonZero, Unsigned, U10};
use crate::Fix;

/// Domain specific extensions to the `Fix` type as it's used in this project.
pub trait FixExt: Sized {
    /// This precision's equivalent of 1.
    const ONE: Self;

    /// Same result as [`MulDiv::mul_div_floor`], through the crate's own
    /// kernel where one exists for `Self`.
    #[doc(hidden)]
    fn mul_div_floor_kernel(self, num: Self, denom: Self) -> Option<Self>
    where
        Self: MulDiv<Output = Self>,
    {
        self.mul_div_floor(num, denom)
    }

    /// Same result as [`MulDiv::mul_div_ceil`], through the crate's own
    /// kernel where one exists for `Self`.
    #[doc(hidden)]
    fn mul_div_ceil_kernel(self, num: Self, denom: Self) -> Option<Self>
    where
        Self: MulDiv<Output = Self>,
    {
        self.mul_div_ceil(num, denom)
    }
}

macro_rules! impl_fix_ext {
    ($bits:ident $(, $floor:ident, $ceil:ident)?) => {
        paste! {
            impl<U> FixExt for Fix<$bits, U10, NInt<U>>
            where
                U: Unsigned + NonZero,
            {
                const ONE: Self =
                    Fix::constant((10 as $bits).pow(U::U32));

                $(
                    fn mul_div_floor_kernel(self, num: Self, denom: Self) -> Option<Self> {
                        self.$floor(num, denom)
                    }

                    fn mul_div_ceil_kernel(self, num: Self, denom: Self) -> Option<Self> {
                        self.$ceil(num, denom)
                    }
                )?
            }
        }
    };
}

impl_fix_ext!(u8);
impl_fix_ext!(u16);
impl_fix_ext!(u32);
impl_fix_ext!(u64, mul_div_floor, mul_div_ceil);
impl_fix_ext!(u128);
impl_fix_ext!(usize);
impl_fix_ext!(i8);
impl_fix_ext!(i16);
impl_fix_ext!(i32);
impl_fix_ext!(i64, mul_div_floor, mul_div_ceil);
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
                let exp = Exp::to_i32();
                let scale = FloatCore::powi(10f64, exp.saturating_abs());
                if exp.is_negative() {
                    self.bits as f64 / scale
                } else {
                    self.bits as f64 * scale
                }
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
    Bits: MulDivKernel,
{
    /// Converts to another _Exp_ rounding to nearest, returning `None` on
    /// overflow: the way to round a wider intermediate once at the end.
    ///
    /// ```
    /// use fix::prelude::*;
    /// use fix::typenum::{N18, N36};
    /// let source = IFix128::<N36>::new(1_234_567_890_123_456_789_500_000_000_000_000_000i128);
    /// let target = source.checked_convert_round::<N18>();
    /// assert_eq!(target, Some(IFix128::<N18>::new(1_234_567_890_123_456_790i128)));
    /// ```
    pub fn checked_convert_round<ToExp>(self) -> Option<Fix<Bits, U10, ToExp>>
    where
        Fix<Bits, U10, ToExp>: FixExt,
    {
        Fix::<Bits, U10, ToExp>::one().mul_div_round(self, Self::one())
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
        Self::new(target_one.bits)
            .mul_div_floor_kernel(self, source_one)
            .map(|fix| Fix::new(fix.bits))
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
        Self::new(target_one.bits)
            .mul_div_ceil_kernel(self, source_one)
            .map(|fix| Fix::new(fix.bits))
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
            self.mul_div_floor_kernel(Self::one(), rhs)
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
            self.mul_div_ceil_kernel(Self::one(), rhs)
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
        self.mul_div_floor_kernel(rhs, Self::one())
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
        self.mul_div_ceil_kernel(rhs, Self::one())
    }
}

macro_rules! impl_ln_exp {
    ($bits:ident, $out:ident, $fix:ident, $out_fix:ident, $n:ident, $one:literal, $below_one:literal, $e:literal, $e_up:literal, $half:literal, $ln_half:literal, $x:literal, $exp_x:literal) => {
        /// Natural logarithm, correctly rounded at this precision. The
        /// result is signed, so it comes back on the signed type of the
        /// same width and exponent. Beyond `N38`, where `10^E` does not
        /// fit `u128`, every call returns `None`.
        impl<U> Fix<$bits, U10, NInt<U>>
        where
            U: Unsigned + NonZero,
        {
            const SCALE: Option<Scale> = Scale::new(U::U32);

            /// `floor(ln(self))`; `None` unless `self > 0` and the result
            /// fits the output type.
            ///
            /// ```
            /// use fix::prelude::*;
            #[doc = concat!("use fix::typenum::", stringify!($n), ";")]
            #[doc = concat!("let e = ", stringify!($fix), "::<", stringify!($n), ">::new(", stringify!($e), stringify!($bits), ");")]
            #[doc = concat!("assert_eq!(e.ln_floor(), Some(", stringify!($out_fix), "::<", stringify!($n), ">::new(", stringify!($below_one), stringify!($out), ")));")]
            /// ```
            pub fn ln_floor(self) -> Option<Fix<$out, U10, NInt<U>>> {
                self.ln(Round::Floor)
            }

            /// `ceil(ln(self))`; `None` unless `self > 0` and the result
            /// fits the output type.
            ///
            /// ```
            /// use fix::prelude::*;
            #[doc = concat!("use fix::typenum::", stringify!($n), ";")]
            #[doc = concat!("let e = ", stringify!($fix), "::<", stringify!($n), ">::new(", stringify!($e), stringify!($bits), ");")]
            #[doc = concat!("assert_eq!(e.ln_ceil(), Some(", stringify!($out_fix), "::<", stringify!($n), ">::new(", stringify!($one), stringify!($out), ")));")]
            /// ```
            pub fn ln_ceil(self) -> Option<Fix<$out, U10, NInt<U>>> {
                self.ln(Round::Ceil)
            }

            /// `round(ln(self))`; `None` unless `self > 0` and the result
            /// fits the output type.
            ///
            /// ```
            /// use fix::prelude::*;
            #[doc = concat!("use fix::typenum::", stringify!($n), ";")]
            #[doc = concat!("let half = ", stringify!($fix), "::<", stringify!($n), ">::new(", stringify!($half), stringify!($bits), ");")]
            #[doc = concat!("assert_eq!(half.ln_round(), Some(", stringify!($out_fix), "::<", stringify!($n), ">::new(", stringify!($ln_half), stringify!($out), ")));")]
            /// ```
            pub fn ln_round(self) -> Option<Fix<$out, U10, NInt<U>>> {
                self.ln(Round::Nearest)
            }

            fn ln(self, round: Round) -> Option<Fix<$out, U10, NInt<U>>> {
                let scale = Self::SCALE.as_ref()?;
                let bits = u128::try_from(self.bits).ok().filter(|&n| n > 0)?;
                let result = transcendental::ln(bits, scale, round)?;
                $out::try_from(result).ok().map(Fix::new)
            }
        }

        /// Exponential, correctly rounded at this precision. Beyond `N38`,
        /// where `10^E` does not fit `u128`, every call returns `None`.
        impl<U> Fix<$bits, U10, NInt<U>>
        where
            U: Unsigned + NonZero,
        {
            /// `floor(exp(self))`; `None` when the result overflows.
            ///
            /// ```
            /// use fix::prelude::*;
            #[doc = concat!("use fix::typenum::", stringify!($n), ";")]
            #[doc = concat!("let one = ", stringify!($fix), "::<", stringify!($n), ">::new(", stringify!($one), stringify!($bits), ");")]
            #[doc = concat!("assert_eq!(one.exp_floor(), Some(", stringify!($fix), "::<", stringify!($n), ">::new(", stringify!($e), stringify!($bits), ")));")]
            /// ```
            pub fn exp_floor(self) -> Option<Self> {
                self.exp(Round::Floor)
            }

            /// `ceil(exp(self))`; `None` when the result overflows.
            ///
            /// ```
            /// use fix::prelude::*;
            #[doc = concat!("use fix::typenum::", stringify!($n), ";")]
            #[doc = concat!("let one = ", stringify!($fix), "::<", stringify!($n), ">::new(", stringify!($one), stringify!($bits), ");")]
            #[doc = concat!("assert_eq!(one.exp_ceil(), Some(", stringify!($fix), "::<", stringify!($n), ">::new(", stringify!($e_up), stringify!($bits), ")));")]
            /// ```
            pub fn exp_ceil(self) -> Option<Self> {
                self.exp(Round::Ceil)
            }

            /// `round(exp(self))`; `None` when the result overflows.
            ///
            /// ```
            /// use fix::prelude::*;
            #[doc = concat!("use fix::typenum::", stringify!($n), ";")]
            #[doc = concat!("let x = ", stringify!($fix), "::<", stringify!($n), ">::new(", stringify!($x), stringify!($bits), ");")]
            #[doc = concat!("assert_eq!(x.exp_round(), Some(", stringify!($fix), "::<", stringify!($n), ">::new(", stringify!($exp_x), stringify!($bits), ")));")]
            /// ```
            pub fn exp_round(self) -> Option<Self> {
                self.exp(Round::Nearest)
            }

            fn exp(self, round: Round) -> Option<Self> {
                let scale = Self::SCALE.as_ref()?;
                // A u128 at or above 2^127 has no i128 form, and None is also
                // its answer: x >= 2^127 / 10^38 > 1.7 and e^1.7 10^E > 2^128
                // at every E, so such an argument always overflows.
                let bits = i128::try_from(self.bits).ok()?;
                let result = transcendental::exp(bits, scale, round)?;
                $bits::try_from(result).ok().map(Fix::new)
            }
        }
    };
}

impl_ln_exp!(u64, i64, UFix64, IFix64, N9, 1_000_000_000, 999_999_999, 2_718_281_828, 2_718_281_829, 500_000_000, -693_147_181, 1_000_000_000, 2_718_281_828);
impl_ln_exp!(i64, i64, IFix64, IFix64, N9, 1_000_000_000, 999_999_999, 2_718_281_828, 2_718_281_829, 500_000_000, -693_147_181, -1_000_000_000, 367_879_441);
impl_ln_exp!(u128, i128, UFix128, IFix128, N18, 1_000_000_000_000_000_000, 999_999_999_999_999_999, 2_718_281_828_459_045_235, 2_718_281_828_459_045_236, 500_000_000_000_000_000, -693_147_180_559_945_309, 1_000_000_000_000_000_000, 2_718_281_828_459_045_235);
impl_ln_exp!(i128, i128, IFix128, IFix128, N18, 1_000_000_000_000_000_000, 999_999_999_999_999_999, 2_718_281_828_459_045_235, 2_718_281_828_459_045_236, 500_000_000_000_000_000, -693_147_180_559_945_309, -1_000_000_000_000_000_000, 367_879_441_171_442_322);

/// `checked_convert`, `div_floor` and the other scale helpers for
/// `Fix<u128>` and `Fix<i128>`, which `muldiv` does not cover.
pub trait MulDivExt: Sized {
    /// The underlying integer type.
    type Bits;
    /// Converts to another _Exp_ rounding down, returning `None` on overflow.
    fn checked_convert<ToExp>(self) -> Option<Fix<Self::Bits, U10, ToExp>>
    where
        Fix<Self::Bits, U10, ToExp>: FixExt;
    /// Converts to another _Exp_ rounding up, returning `None` on overflow.
    fn checked_convert_ceil<ToExp>(self) -> Option<Fix<Self::Bits, U10, ToExp>>
    where
        Fix<Self::Bits, U10, ToExp>: FixExt;
    /// Divides at the same precision rounding down; `None` on overflow or zero.
    fn div_floor(self, rhs: Self) -> Option<Self>;
    /// Divides at the same precision rounding up; `None` on overflow or zero.
    fn div_ceil(self, rhs: Self) -> Option<Self>;
    /// Multiplies at the same precision rounding down; `None` on overflow.
    fn mul_floor(self, rhs: Self) -> Option<Self>;
    /// Multiplies at the same precision rounding up; `None` on overflow.
    fn mul_ceil(self, rhs: Self) -> Option<Self>;
}

macro_rules! impl_mul_div_ext {
    ($bits:ident) => {
        impl<Exp> MulDivExt for Fix<$bits, U10, Exp>
        where
            Self: FixExt,
        {
            type Bits = $bits;

            fn checked_convert<ToExp>(self) -> Option<Fix<$bits, U10, ToExp>>
            where
                Fix<$bits, U10, ToExp>: FixExt,
            {
                Fix::<$bits, U10, ToExp>::one().mul_div_floor(self, Self::one())
            }

            fn checked_convert_ceil<ToExp>(self) -> Option<Fix<$bits, U10, ToExp>>
            where
                Fix<$bits, U10, ToExp>: FixExt,
            {
                Fix::<$bits, U10, ToExp>::one().mul_div_ceil(self, Self::one())
            }

            fn div_floor(self, rhs: Self) -> Option<Self> {
                if rhs == Self::zero() {
                    None
                } else {
                    self.mul_div_floor(Self::one(), rhs)
                }
            }

            fn div_ceil(self, rhs: Self) -> Option<Self> {
                if rhs == Self::zero() {
                    None
                } else {
                    self.mul_div_ceil(Self::one(), rhs)
                }
            }

            fn mul_floor(self, rhs: Self) -> Option<Self> {
                self.mul_div_floor(rhs, Self::one())
            }

            fn mul_ceil(self, rhs: Self) -> Option<Self> {
                self.mul_div_ceil(rhs, Self::one())
            }
        }
    };
}

impl_mul_div_ext!(u128);
impl_mul_div_ext!(i128);

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

    #[test]
    fn exp_of_a_u128_at_or_above_2_pow_127_overflows_at_every_exponent() {
        use crate::aliases::decimal::UFix128;
        use crate::typenum::{N1, N38};
        macro_rules! check {
            ($n:ident) => {
                let top = UFix128::<$n>::new(u128::MAX).ln_floor().unwrap().bits;
                assert!(top > 0 && (top as u128) < 1 << 127, "{}: {top}", stringify!($n));
                assert!(UFix128::<$n>::new(top as u128).exp_floor().is_some());
                assert_eq!(UFix128::<$n>::new(1 << 127).exp_floor(), None);
                assert_eq!(UFix128::<$n>::new(u128::MAX).exp_ceil(), None);
            };
        }
        check!(N38);
        check!(N1);
    }

    #[test]
    fn ln_and_exp_are_none_beyond_n38() {
        use crate::aliases::decimal::{IFix128, UFix128};
        use crate::typenum::{N38, N39};
        assert!(UFix128::<N38>::new(10u128.pow(38)).ln_round().is_some());
        assert!(IFix128::<N38>::new(1).exp_round().is_some());
        assert_eq!(UFix128::<N39>::new(1).ln_round(), None);
        assert_eq!(IFix128::<N39>::new(1).exp_round(), None);
        assert_eq!(IFix128::<N39>::new(0).exp_floor(), None);
        assert!(crate::transcendental::Scale::new(39).is_none());
    }
}
