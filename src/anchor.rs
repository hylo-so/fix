use crate::typenum::{Integer, Unsigned};
use crate::Fix;
use anchor_lang::idl::IdlBuild;
use anchor_lang::prelude::{borsh, AnchorDeserialize, AnchorSerialize, ErrorCode, Space};

/// A dumbed down value version of `Fix` with no base or exponent types.
/// Intended for storage on chain in Solana accounts, as it implements Borsh serde
/// should play nicely with Anchor's IDL generator.
#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub struct FixValue<Bits> {
    pub bits: Bits,
    pub base: u64,
    pub exp: i64,
}

impl<Bits> FixValue<Bits> {
    pub fn new(bits: Bits, base: u64, exp: i64) -> Self {
        Self { bits, base, exp }
    }
}

impl<Bits, Base, Exp> From<Fix<Bits, Base, Exp>> for FixValue<Bits>
where
    Base: Unsigned,
    Exp: Integer,
{
    fn from(fix: Fix<Bits, Base, Exp>) -> FixValue<Bits> {
        FixValue {
            bits: fix.bits,
            base: Base::to_u64(),
            exp: Exp::to_i64(),
        }
    }
}

impl<Bits, Base, Exp> TryInto<Fix<Bits, Base, Exp>> for FixValue<Bits>
where
    Base: Unsigned,
    Exp: Integer,
{
    type Error = anchor_lang::error::Error;
    fn try_into(self) -> anchor_lang::Result<Fix<Bits, Base, Exp>> {
        let base = Base::to_u64();
        let exp = Exp::to_i64();
        if base == self.base && exp == self.exp {
            Ok(Fix::new(self.bits))
        } else {
            Err(ErrorCode::AccountDidNotDeserialize.into())
        }
    }
}

impl<Bits> AnchorSerialize for FixValue<Bits>
where
    Bits: AnchorSerialize,
{
    fn serialize<W>(&self, w: &mut W) -> Result<(), borsh::maybestd::io::Error>
    where
        W: borsh::maybestd::io::Write,
    {
        self.bits
            .serialize(w)
            .and_then(|()| self.base.serialize(w))
            .and_then(|()| self.exp.serialize(w))
    }
}

impl<Bits> AnchorDeserialize for FixValue<Bits>
where
    Bits: AnchorDeserialize,
{
    fn deserialize_reader<R>(r: &mut R) -> Result<Self, borsh::maybestd::io::Error>
    where
        R: borsh::maybestd::io::Read,
    {
        let bits: Bits = AnchorDeserialize::deserialize_reader(r)?;
        let base: u64 = AnchorDeserialize::deserialize_reader(r)?;
        let exp: i64 = AnchorDeserialize::deserialize_reader(r)?;
        Ok(FixValue { bits, base, exp })
    }
}

impl<Bits> IdlBuild for FixValue<Bits> {}

macro_rules! impl_init_space {
    ($ty:ident) => {
        impl Space for FixValue<$ty> {
            const INIT_SPACE: usize = core::mem::size_of::<$ty>();
        }
    };
}

impl_init_space!(u8);
impl_init_space!(u16);
impl_init_space!(u32);
impl_init_space!(u64);
impl_init_space!(u128);
impl_init_space!(usize);
impl_init_space!(i8);
impl_init_space!(i16);
impl_init_space!(i32);
impl_init_space!(i64);
impl_init_space!(i128);
impl_init_space!(isize);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aliases::si::Kilo;
    use anyhow::Result;
    use borsh::to_vec;

    #[test]
    fn round_trip_vfix() -> Result<()> {
        let start = Kilo::new(6900u64);
        let there: FixValue<u64> = start.into();
        let back: Kilo<u64> = there.try_into()?;
        assert_eq!(there, FixValue::new(6900u64, 10, 3));
        Ok(assert_eq!(start, back))
    }

    #[test]
    fn serialize() -> Result<()> {
        let start = FixValue::new(89001u32, 10, -2);
        let bytes = to_vec(&start)?;
        let back = AnchorDeserialize::deserialize(&mut bytes.as_slice())?;
        Ok(assert_eq!(start, back))
    }
}
