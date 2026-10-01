#![allow(dead_code)]

mod gen {
    #[allow(unused_imports)]
    use unity::prelude::*;

    #[unity::class(namespace = "Test", name = "OffsetDummy")]
    pub struct OffsetDummy {
        #[offset(16)]
        #[rename(name = "m_Value")]
        pub m_value: i32,
        #[offset(24)]
        #[rename(name = "m_Ratio")]
        pub m_ratio: f32,
        #[rename(name = "m_NoOffset")]
        pub m_no_offset: i32,
        #[static_field]
        #[rename(name = "s_Count")]
        pub s_count: i32,
    }
}

use gen::{IOffsetDummy, OffsetDummy};

fn _exercise(d: OffsetDummy) {
    let _ = d.m_value();
    d.set_m_value(7);
    let _ = d.m_ratio();
    d.set_m_ratio(1.0);
    let _ = d.m_no_offset();
    d.set_m_no_offset(3);
    let _ = OffsetDummy::s_count();
    OffsetDummy::set_s_count(9);
}
