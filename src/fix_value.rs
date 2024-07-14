use crate::typenum::{Integer, Unsigned};
use crate::Fix;
use anchor_lang::prelude::{borsh, AnchorDeserialize, AnchorSerialize, ErrorCode, InitSpace};
use paste::paste;

macro_rules! impl_fix_value {
    ($sign:ident, $bits:expr) => {
        paste! {
           /// A dumbed down version of `Fix` with a concrete `bits` type.
           /// Only intended for serialized storage in Solana accounts.
            #[derive(PartialEq, Eq, Copy, Clone, Debug, AnchorSerialize, AnchorDeserialize, InitSpace)]
            pub struct [<$sign FixValue $bits>] {
                bits: [<$sign:lower $bits>],
                base: u8,
                exp: i8,
            }

            impl [<$sign FixValue $bits>] {
                pub fn new(bits: [<$sign:lower $bits>], base: u8, exp: i8) -> Self {
                    Self { bits, base, exp }
                }
            }

            impl<Bits, Base, Exp> From<Fix<Bits, Base, Exp>> for [<$sign FixValue $bits>]
            where
                Bits: Into<[<$sign:lower $bits>]>,
                Base: Unsigned,
                Exp: Integer,
            {
                fn from(fix: Fix<Bits, Base, Exp>) -> Self {
                    Self {
                        bits: fix.bits.into(),
                        base: Base::to_u8(),
                        exp: Exp::to_i8(),
                    }
                }
            }

            impl<Bits, Base, Exp> TryInto<Fix<Bits, Base, Exp>> for [<$sign FixValue $bits>]
            where
                Bits: From<[<$sign:lower $bits>]>,
                Base: Unsigned,
                Exp: Integer,
            {
                type Error = anchor_lang::error::Error;
                fn try_into(self) -> anchor_lang::Result<Fix<Bits, Base, Exp>> {
                    let base = Base::to_u8();
                    let exp = Exp::to_i8();
                    if base == self.base && exp == self.exp {
                        Ok(Fix::new(Bits::from(self.bits)))
                    } else {
                        Err(ErrorCode::AccountDidNotDeserialize.into())
                    }
                }
            }
        }
    };
}

impl_fix_value!(U, 8);
impl_fix_value!(U, 16);
impl_fix_value!(U, 32);
impl_fix_value!(U, 64);
impl_fix_value!(U, 128);
impl_fix_value!(I, 8);
impl_fix_value!(I, 16);
impl_fix_value!(I, 32);
impl_fix_value!(I, 64);
impl_fix_value!(I, 128);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aliases::si::Kilo;
    use anyhow::Result;
    use borsh::to_vec;

    macro_rules! fix_value_tests {
        ($sign:ident, $bits:expr) => {
            paste! {
                #[test]
                fn [<roundtrip_into_ $sign:lower $bits>]() -> Result<()> {
                    let start = Kilo::new([<69 $sign:lower $bits>]);
                    let there: [<$sign FixValue $bits>] = start.into();
                    let back: Kilo<[<$sign:lower $bits>]> = there.try_into()?;
                    assert_eq!(there, [<$sign FixValue $bits>]::new(69, 10, 3));
                    Ok(assert_eq!(start, back))
                }

                #[test]
                fn [<roundtrip_serialize_ $sign:lower $bits>]() -> Result<()> {
                    let start = [<$sign FixValue $bits>]::new(20, 10, -2);
                    let bytes = to_vec(&start)?;
                    let back = AnchorDeserialize::deserialize(&mut bytes.as_slice())?;
                    Ok(assert_eq!(start, back))
                }
            }
        };
    }

    fix_value_tests!(U, 8);
    fix_value_tests!(U, 16);
    fix_value_tests!(U, 32);
    fix_value_tests!(U, 64);
    fix_value_tests!(U, 128);
    fix_value_tests!(I, 8);
    fix_value_tests!(I, 16);
    fix_value_tests!(I, 32);
    fix_value_tests!(I, 64);
    fix_value_tests!(I, 128);
}
