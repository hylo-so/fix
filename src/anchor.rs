use crate::typenum::{Integer, Unsigned};
use crate::Fix;
use anchor_lang::idl::IdlBuild;
use anchor_lang::prelude::{borsh, AnchorDeserialize, AnchorSerialize, ErrorCode, Space};

/// A "flattened" value-space version of `Fix` with no dependence on typenum.
/// Intended to be used as a stored type on chain, as it's compatible with Anchor's
/// serde and IDL generator.
pub struct VFix<Bits> {
    pub bits: Bits,
    pub base: u64,
    pub exp: i64,
}

impl<Bits, Base, Exp> From<Fix<Bits, Base, Exp>> for VFix<Bits>
where
    Base: Unsigned,
    Exp: Integer,
{
    fn from(fix: Fix<Bits, Base, Exp>) -> VFix<Bits> {
        VFix {
            bits: fix.bits,
            base: Base::to_u64(),
            exp: Exp::to_i64(),
        }
    }
}

impl<Bits, Base, Exp> TryInto<Fix<Bits, Base, Exp>> for VFix<Bits>
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

impl<Bits> AnchorSerialize for VFix<Bits>
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

impl<Bits> AnchorDeserialize for VFix<Bits>
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
        Ok(VFix { bits, base, exp })
    }
}

impl<Bits> IdlBuild for VFix<Bits> {}

macro_rules! vfix_init_space {
    ($ty:ident) => {
        impl Space for VFix<$ty> {
            const INIT_SPACE: usize = core::mem::size_of::<$ty>();
        }
    };
}

vfix_init_space!(u8);
vfix_init_space!(u16);
vfix_init_space!(u32);
vfix_init_space!(u64);
vfix_init_space!(u128);
vfix_init_space!(usize);
vfix_init_space!(i8);
vfix_init_space!(i16);
vfix_init_space!(i32);
vfix_init_space!(i64);
vfix_init_space!(i128);
vfix_init_space!(isize);
