use crate::typenum::{Integer, U10};
use crate::Fix;
use anchor_lang::prelude::{borsh, AnchorDeserialize, AnchorSerialize, InitSpace};
use paste::paste;

macro_rules! impl_fix_value {
    ($sign:ident, $bits:expr) => {
        paste! {
           /// A value-space `Fix` where base is always 10 and bits are a concrete type.
           /// Intended for serialized storage in Solana accounts where generics won't work.
            #[derive(PartialEq, Eq, Copy, Clone, Debug, AnchorSerialize, AnchorDeserialize, InitSpace)]
            pub struct [<$sign FixValue $bits>] {
                pub bits: [<$sign:lower $bits>],
                pub exp: i8,
            }

            impl [<$sign FixValue $bits>] {
                pub fn new(bits: [<$sign:lower $bits>], exp: i8) -> Self {
                    Self { bits, exp }
                }
            }

            impl<Bits, Exp> From<Fix<Bits, U10, Exp>> for [<$sign FixValue $bits>]
            where
                Bits: Into<[<$sign:lower $bits>]>,
                Exp: Integer,
            {
                fn from(fix: Fix<Bits, U10, Exp>) -> Self {
                    Self {
                        bits: fix.bits.into(),
                        exp: Exp::to_i8(),
                    }
                }
            }

            impl<Bits, Exp> From<[<$sign FixValue $bits>]> for Fix<Bits, U10, Exp>
            where
                Bits: From<[<$sign:lower $bits>]>,
                Exp: Integer,
            {
              fn from(value: [<$sign FixValue $bits>]) -> Fix<Bits, U10, Exp> {
                Fix::new(value.bits.into())
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
                    let back: Kilo<[<$sign:lower $bits>]> = there.into();
                    assert_eq!(there, [<$sign FixValue $bits>]::new(69, 3));
                    Ok(assert_eq!(start, back))
                }

                #[test]
                fn [<roundtrip_serialize_ $sign:lower $bits>]() -> Result<()> {
                    let start = [<$sign FixValue $bits>]::new(20, -2);
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
