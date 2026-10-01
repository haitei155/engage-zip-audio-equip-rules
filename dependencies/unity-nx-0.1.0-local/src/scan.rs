use skyline::hooks::{getRegionAddress, Region};

#[inline]
pub fn module_base() -> usize {
    use std::sync::OnceLock;
    static BASE: OnceLock<usize> = OnceLock::new();
    *BASE.get_or_init(|| unsafe { getRegionAddress(Region::Text) as usize })
}

#[inline]
pub fn callable_ptr(ptr: *mut u8) -> *mut u8 {
    if ptr.is_null() {
        return ptr;
    }
    unsafe {
        let w = ptr as *const u32;
        let w0 = w.read();
        let w1 = w.add(1).read();
        // add x0, x0, #0x10  followed by  b <imm26>
        if w0 == 0x9100_4000 && (w1 >> 26) == 0b0001_01 {
            let off = (((w1 & 0x03ff_ffff) as i32) << 6 >> 6) * 4;
            return ptr.add(4).offset(off as isize);
        }
        ptr
    }
}

pub fn get_text() -> &'static [u8] {
    unsafe {
        let ptr = getRegionAddress(Region::Text) as *const u8;
        let size = (getRegionAddress(Region::Rodata) as usize) - (ptr as usize);
        std::slice::from_raw_parts(ptr, size)
    }
}
