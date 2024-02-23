/// Base-2 types.
pub mod binary {
    use typenum::U2;

    use crate::Fix;

    pub type UFix8<Exp> = Fix<u8, U2, Exp>;
    pub type UFix16<Exp> = Fix<u16, U2, Exp>;
    pub type UFix32<Exp> = Fix<u32, U2, Exp>;
    pub type UFix64<Exp> = Fix<u64, U2, Exp>;
    pub type UFixSize<Exp> = Fix<usize, U2, Exp>;

    #[cfg(feature = "i128")]
    pub type UFix128<Exp> = Fix<u128, U2, Exp>;

    pub type IFix8<Exp> = Fix<i8, U2, Exp>;
    pub type IFix16<Exp> = Fix<i16, U2, Exp>;
    pub type IFix32<Exp> = Fix<i32, U2, Exp>;
    pub type IFix64<Exp> = Fix<i64, U2, Exp>;
    pub type IFixSize<Exp> = Fix<isize, U2, Exp>;

    #[cfg(feature = "i128")]
    pub type IFix128<Exp> = Fix<i128, U2, Exp>;
}

/// Base-10 types.
pub mod decimal {
    use typenum::U10;

    use crate::Fix;

    pub type UFix8<Exp> = Fix<u8, U10, Exp>;
    pub type UFix16<Exp> = Fix<u16, U10, Exp>;
    pub type UFix32<Exp> = Fix<u32, U10, Exp>;
    pub type UFix64<Exp> = Fix<u64, U10, Exp>;
    pub type UFixSize<Exp> = Fix<usize, U10, Exp>;

    #[cfg(feature = "i128")]
    pub type UFix128<Exp> = Fix<u128, U10, Exp>;

    pub type IFix8<Exp> = Fix<i8, U10, Exp>;
    pub type IFix16<Exp> = Fix<i16, U10, Exp>;
    pub type IFix32<Exp> = Fix<i32, U10, Exp>;
    pub type IFix64<Exp> = Fix<i64, U10, Exp>;
    pub type IFixSize<Exp> = Fix<isize, U10, Exp>;

    #[cfg(feature = "i128")]
    pub type IFix128<Exp> = Fix<i128, U10, Exp>;
}

/// SI prefixes.
pub mod si {
    use typenum::{N1, N12, N15, N18, N2, N21, N24, N3, N6, N9};
    use typenum::{P1, P12, P15, P18, P2, P21, P24, P3, P6, P9};
    use typenum::{U10, Z0};

    use crate::Fix;

    /** 10<sup>-24</sup> */
    pub type Yocto<Bits> = Fix<Bits, U10, N24>;
    /** 10<sup>-21</sup> */
    pub type Zepto<Bits> = Fix<Bits, U10, N21>;
    /** 10<sup>-18</sup> */
    pub type Atto<Bits> = Fix<Bits, U10, N18>;
    /** 10<sup>-15</sup> */
    pub type Femto<Bits> = Fix<Bits, U10, N15>;
    /** 10<sup>-12</sup> */
    pub type Pico<Bits> = Fix<Bits, U10, N12>;
    /** 10<sup>-9</sup> */
    pub type Nano<Bits> = Fix<Bits, U10, N9>;
    /** 10<sup>-6</sup> */
    pub type Micro<Bits> = Fix<Bits, U10, N6>;
    /** 10<sup>-3</sup> */
    pub type Milli<Bits> = Fix<Bits, U10, N3>;
    /** 10<sup>-2</sup> */
    pub type Centi<Bits> = Fix<Bits, U10, N2>;
    /** 10<sup>-1</sup> */
    pub type Deci<Bits> = Fix<Bits, U10, N1>;

    /** 10<sup>0</sup> */
    pub type Unit<Bits> = Fix<Bits, U10, Z0>;

    /** 10<sup>1</sup> */
    pub type Deca<Bits> = Fix<Bits, U10, P1>;
    /** 10<sup>2</sup> */
    pub type Hecto<Bits> = Fix<Bits, U10, P2>;
    /** 10<sup>3</sup> */
    pub type Kilo<Bits> = Fix<Bits, U10, P3>;
    /** 10<sup>6</sup> */
    pub type Mega<Bits> = Fix<Bits, U10, P6>;
    /** 10<sup>9</sup> */
    pub type Giga<Bits> = Fix<Bits, U10, P9>;
    /** 10<sup>12</sup> */
    pub type Tera<Bits> = Fix<Bits, U10, P12>;
    /** 10<sup>15</sup> */
    pub type Peta<Bits> = Fix<Bits, U10, P15>;
    /** 10<sup>18</sup> */
    pub type Exa<Bits> = Fix<Bits, U10, P18>;
    /** 10<sup>21</sup> */
    pub type Zeta<Bits> = Fix<Bits, U10, P21>;
    /** 10<sup>24</sup> */
    pub type Yotta<Bits> = Fix<Bits, U10, P24>;
}

/// IEC prefixes.
pub mod iec {
    use typenum::{P10, P20, P30, P40, P50, P60, P70, P80};
    use typenum::{U2, Z0};

    use crate::Fix;

    /** 2<sup>0</sup> */
    pub type Unit<Bits> = Fix<Bits, U2, Z0>;

    /** 2<sup>10</sup> */
    pub type Kibi<Bits> = Fix<Bits, U2, P10>;
    /** 2<sup>20</sup> */
    pub type Mebi<Bits> = Fix<Bits, U2, P20>;
    /** 2<sup>30</sup> */
    pub type Gibi<Bits> = Fix<Bits, U2, P30>;
    /** 2<sup>40</sup> */
    pub type Tebi<Bits> = Fix<Bits, U2, P40>;
    /** 2<sup>50</sup> */
    pub type Pebi<Bits> = Fix<Bits, U2, P50>;
    /** 2<sup>60</sup> */
    pub type Exbi<Bits> = Fix<Bits, U2, P60>;
    /** 2<sup>70</sup> */
    pub type Zebi<Bits> = Fix<Bits, U2, P70>;
    /** 2<sup>80</sup> */
    pub type Yobi<Bits> = Fix<Bits, U2, P80>;
}
