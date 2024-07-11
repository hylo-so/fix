use crate::typenum::{Integer, Unsigned};
use crate::Fix;
use anchor_lang::prelude::*;

#[derive(PartialEq, AnchorSerialize, AnchorDeserialize, Eq, Copy, Clone, Debug)]
pub struct FixU64 {
    bits: u64,
    base: u8,
    exp: i8,
}

impl FixU64 {
    pub fn new(bits: u64, base: u8, exp: i8) -> Self {
        Self { bits, base, exp }
    }
}

impl<Base, Exp> From<Fix<u64, Base, Exp>> for FixU64
where
    Base: Unsigned,
    Exp: Integer,
{
    fn from(fix: Fix<u64, Base, Exp>) -> FixU64 {
        FixU64 {
            bits: fix.bits,
            base: Base::to_u8(),
            exp: Exp::to_i8(),
        }
    }
}

impl<Base, Exp> TryInto<Fix<u64, Base, Exp>> for FixU64
where
    Base: Unsigned,
    Exp: Integer,
{
    type Error = anchor_lang::error::Error;
    fn try_into(self) -> anchor_lang::Result<Fix<u64, Base, Exp>> {
        let base = Base::to_u8();
        let exp = Exp::to_i8();
        if base == self.base && exp == self.exp {
            Ok(Fix::new(self.bits))
        } else {
            Err(ErrorCode::AccountDidNotDeserialize.into())
        }
    }
}

impl Space for FixU64 {
    const INIT_SPACE: usize = 8 + 1 + 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aliases::si::Kilo;
    use anyhow::Result;
    use borsh::to_vec;

    #[test]
    fn roundtrip_into_fixval() -> Result<()> {
        let start = Kilo::new(6900u64);
        let there: FixU64 = start.into();
        let back: Kilo<u64> = there.try_into()?;
        assert_eq!(there, FixU64::new(6900u64, 10u8, 3i8));
        Ok(assert_eq!(start, back))
    }

    #[test]
    fn roundtrip_serialize_fixval() -> Result<()> {
        let start = FixU64::new(89001u64, 10u8, -2i8);
        let bytes = to_vec(&start)?;
        let back = AnchorDeserialize::deserialize(&mut bytes.as_slice())?;
        Ok(assert_eq!(start, back))
    }
}
