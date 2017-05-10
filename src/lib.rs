#![no_std]

pub extern crate typenum;

/// Type aliases.
pub mod aliases;

use core::cmp::{PartialEq, Eq, PartialOrd, Ord, Ordering};
use core::fmt::{Debug, Formatter, Error};
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::ops::{Neg, Add, Sub, Mul, Div, Rem};
use core::ops::{AddAssign, SubAssign, MulAssign, DivAssign, RemAssign};

use typenum::{Bit, Unsigned, Integer, Abs, IsLess, Sum, Diff, AbsVal, Z0};

/// Fixed-point number representing _Bits × Base ^Exp_.
///
/// - `Bits` is an integer primitive type.
/// - `Base` is an [`Unsigned`] type-level integer.
/// - `Exp` is a signed type-level [`Integer`].
///
/// [`Unsigned`]: ../typenum/marker_traits/trait.Unsigned.html
/// [`Integer`]: ../typenum/marker_traits/trait.Integer.html
///
/// # Summary of implemented traits
///
/// - `Clone`, `Copy`, `Default`, `Hash`, `Debug`.
/// - `PartialEq`, `Eq` between the same *Bits*, *Base* and *Exp*.
/// - `PartialOrd`, `Ord` between the same *Bits*, *Base* and *Exp*.
/// - `Neg` where *Bits* does.
/// - `Add`, `Sub` between the same *Bits*, *Base* and *Exp*.
/// - `Mul`, `Div`, `Rem` between the same *Bits* and *Base*.
/// - `Mul`, `Div`, `Rem` between `Fix` and `Bits`.
/// - `AddAssign`, `SubAssign` between the same *Bits*, *Base* and *Exp*.
/// - `MulAssign`, `DivAssign`, `RemAssign` between `Fix` and `Bits`.
/// - `RemAssign` between the same *Bits* and *Base*.
pub struct Fix<Bits, Base, Exp> {
    bits: Bits,
    marker: PhantomData<(Base, Exp)>,
}

pub trait FromType {
    fn from_type<U>() -> Self where U: Unsigned;
}

impl FromType for u8 { fn from_type<U>() -> Self where U: Unsigned { U::to_u8() } }
impl FromType for u16 { fn from_type<U>() -> Self where U: Unsigned { U::to_u16() } }
impl FromType for u32 { fn from_type<U>() -> Self where U: Unsigned { U::to_u32() } }
impl FromType for u64 { fn from_type<U>() -> Self where U: Unsigned { U::to_u64() } }
impl FromType for usize { fn from_type<U>() -> Self where U: Unsigned { U::to_usize() } }

impl FromType for i8 { fn from_type<U>() -> Self where U: Unsigned { U::to_i8() } }
impl FromType for i16 { fn from_type<U>() -> Self where U: Unsigned { U::to_i16() } }
impl FromType for i32 { fn from_type<U>() -> Self where U: Unsigned { U::to_i32() } }
impl FromType for i64 { fn from_type<U>() -> Self where U: Unsigned { U::to_i64() } }
impl FromType for isize { fn from_type<U>() -> Self where U: Unsigned { U::to_isize() } }

// TODO: Use num crate for this?
pub trait Pow {
    fn pow(self, exp: u32) -> Self;
}

impl Pow for u8 { fn pow(self, exp: u32) -> Self { self.pow(exp) } }
impl Pow for u16 { fn pow(self, exp: u32) -> Self { self.pow(exp) } }
impl Pow for u32 { fn pow(self, exp: u32) -> Self { self.pow(exp) } }
impl Pow for u64 { fn pow(self, exp: u32) -> Self { self.pow(exp) } }
impl Pow for usize { fn pow(self, exp: u32) -> Self { self.pow(exp) } }

impl Pow for i8 { fn pow(self, exp: u32) -> Self { self.pow(exp) } }
impl Pow for i16 { fn pow(self, exp: u32) -> Self { self.pow(exp) } }
impl Pow for i32 { fn pow(self, exp: u32) -> Self { self.pow(exp) } }
impl Pow for i64 { fn pow(self, exp: u32) -> Self { self.pow(exp) } }
impl Pow for isize { fn pow(self, exp: u32) -> Self { self.pow(exp) } }

impl<Bits, Base, Exp> Fix<Bits, Base, Exp> {
    /// Creates a new number.
    pub fn new(bits: Bits) -> Self {
        Fix { bits, marker: PhantomData }
    }

    /// Returns the underlying bits.
    pub fn into_bits(self) -> Bits {
        self.bits
    }

    /// Converts to another _Exp_.
    pub fn convert<ToExp>(self) -> Fix<Bits, Base, ToExp>
    where
        Bits: FromType + Pow + Mul<Output = Bits> + Div<Output = Bits>,
        Base: Unsigned,
        Exp: Sub<ToExp>,
        Diff<Exp, ToExp>: Abs + IsLess<Z0>,
        AbsVal<Diff<Exp, ToExp>>: Integer
    {
        let base = Bits::from_type::<Base>();
        let diff = AbsVal::<Diff<Exp, ToExp>>::to_i32();
        let inverse = <Diff<Exp, ToExp> as IsLess<Z0>>::Output::to_bool();

        // FIXME: Would like to do this with typenum::Pow, but that
        // seems to result in overflow evaluating requirements.
        let ratio = base.pow(diff as u32);

        if inverse {
            Fix::new(self.bits / ratio)
        } else {
            Fix::new(self.bits * ratio)
        }
    }
}

// The usual traits.

impl<Bits, Base, Exp> Copy for Fix<Bits, Base, Exp> where Bits: Copy { }
impl<Bits, Base, Exp> Clone for Fix<Bits, Base, Exp>
where Bits: Clone {
    fn clone(&self) -> Self {
        Self::new(self.bits.clone())
    }
}

impl<Bits, Base, Exp> Default for Fix<Bits, Base, Exp>
where Bits: Default {
    fn default() -> Self {
        Self::new(Bits::default())
    }
}

impl<Bits, Base, Exp> Hash for Fix<Bits, Base, Exp>
where Bits: Hash {
    fn hash<H>(&self, state: &mut H) where H: Hasher {
        self.bits.hash(state);
    }
}

impl<Bits, Base, Exp> Debug for Fix<Bits, Base, Exp>
where Bits: Debug, Base: Unsigned, Exp: Integer {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "{:?}x{}^{}", self.bits, Base::to_u64(), Exp::to_i64())
    }
}

// Comparison.

impl<Bits, Base, Exp> Eq for Fix<Bits, Base, Exp> where Bits: Eq { }
impl<Bits, Base, Exp> PartialEq for Fix<Bits, Base, Exp>
where Bits: PartialEq {
    fn eq(&self, rhs: &Self) -> bool {
        self.bits == rhs.bits
    }
}

impl<Bits, Base, Exp> PartialOrd for Fix<Bits, Base, Exp>
where Bits: PartialOrd {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        self.bits.partial_cmp(&rhs.bits)
    }
}

impl<Bits, Base, Exp> Ord for Fix<Bits, Base, Exp>
where Bits: Ord {
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.bits.cmp(&rhs.bits)
    }
}

// Arithmetic.

impl<Bits, Base, Exp> Neg for Fix<Bits, Base, Exp>
where Bits: Neg<Output = Bits> {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.bits)
    }
}

impl<Bits, Base, Exp> Add for Fix<Bits, Base, Exp>
where Bits: Add<Output = Bits> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.bits + rhs.bits)
    }
}

impl<Bits, Base, Exp> Sub for Fix<Bits, Base, Exp>
where Bits: Sub<Output = Bits> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.bits - rhs.bits)
    }
}

impl<Bits, Base, LExp, RExp> Mul<Fix<Bits, Base, RExp>> for Fix<Bits, Base, LExp>
where Bits: Mul<Output = Bits>, LExp: Add<RExp> {
    type Output = Fix<Bits, Base, Sum<LExp, RExp>>;
    fn mul(self, rhs: Fix<Bits, Base, RExp>) -> Self::Output {
        Self::Output::new(self.bits * rhs.bits)
    }
}

impl<Bits, Base, LExp, RExp> Div<Fix<Bits, Base, RExp>> for Fix<Bits, Base, LExp>
where Bits: Div<Output = Bits>, LExp: Sub<RExp> {
    type Output = Fix<Bits, Base, Diff<LExp, RExp>>;
    fn div(self, rhs: Fix<Bits, Base, RExp>) -> Self::Output {
        Self::Output::new(self.bits / rhs.bits)
    }
}

impl<Bits, Base, LExp, RExp> Rem<Fix<Bits, Base, RExp>> for Fix<Bits, Base, LExp>
where Bits: Rem<Output = Bits> {
    type Output = Self;
    fn rem(self, rhs: Fix<Bits, Base, RExp>) -> Self {
        Self::new(self.bits % rhs.bits)
    }
}

impl<Bits, Base, Exp> Mul<Bits> for Fix<Bits, Base, Exp>
where Bits: Mul<Output = Bits> {
    type Output = Self;
    fn mul(self, rhs: Bits) -> Self {
        Self::new(self.bits * rhs)
    }
}

impl<Bits, Base, Exp> Div<Bits> for Fix<Bits, Base, Exp>
where Bits: Div<Output = Bits> {
    type Output = Self;
    fn div(self, rhs: Bits) -> Self {
        Self::new(self.bits / rhs)
    }
}

impl<Bits, Base, Exp> Rem<Bits> for Fix<Bits, Base, Exp>
where Bits: Rem<Output = Bits> {
    type Output = Self;
    fn rem(self, rhs: Bits) -> Self {
        Self::new(self.bits % rhs)
    }
}

// Assignment.

impl<Bits, Base, Exp> AddAssign for Fix<Bits, Base, Exp>
where Bits: AddAssign {
    fn add_assign(&mut self, rhs: Self) {
        self.bits += rhs.bits;
    }
}

impl<Bits, Base, Exp> SubAssign for Fix<Bits, Base, Exp>
where Bits: SubAssign {
    fn sub_assign(&mut self, rhs: Self) {
        self.bits -= rhs.bits;
    }
}

impl<Bits, Base, Exp> MulAssign<Bits> for Fix<Bits, Base, Exp>
where Bits: MulAssign {
    fn mul_assign(&mut self, rhs: Bits) {
        self.bits *= rhs;
    }
}

impl<Bits, Base, Exp> DivAssign<Bits> for Fix<Bits, Base, Exp>
where Bits: DivAssign {
    fn div_assign(&mut self, rhs: Bits) {
        self.bits /= rhs;
    }
}

impl<Bits, Base, LExp, RExp> RemAssign<Fix<Bits, Base, RExp>> for Fix<Bits, Base, LExp>
where Bits: RemAssign {
    fn rem_assign(&mut self, rhs: Fix<Bits, Base, RExp>) {
        self.bits %= rhs.bits;
    }
}

impl<Bits, Base, Exp> RemAssign<Bits> for Fix<Bits, Base, Exp>
where Bits: RemAssign {
    fn rem_assign(&mut self, rhs: Bits) {
        self.bits %= rhs;
    }
}
