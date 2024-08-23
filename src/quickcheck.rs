use crate::aliases::decimal::UFix64;
use quickcheck::Arbitrary;

impl<Exp: 'static> Arbitrary for UFix64<Exp> {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        UFix64::new(u64::arbitrary(g))
    }
}
