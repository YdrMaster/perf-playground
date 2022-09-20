use core::ptr::NonNull;
use customizable_buddy::{BuddyAllocator, LinkedListBuddy, UsizeBuddy};
use page_table::{MmuMeta, Sv39};

use crate::layout::MemLayout;

/// 全局页帧分配器。
pub static mut GLOBAL: BuddyAllocator<20, UsizeBuddy, LinkedListBuddy> = BuddyAllocator::new();

/// 建立页分配器。
pub(crate) fn init_global(layout: &MemLayout, dtb_addr: usize) -> usize {
    use dtb_walker::{Dtb, DtbObj, HeaderError::*, Property, WalkOperation::*};
    unsafe {
        GLOBAL.init(
            Sv39::PAGE_BITS,
            NonNull::new_unchecked(layout.start() as *mut u8),
        )
    };
    // 从设备树解析内存信息
    let dtb = unsafe {
        Dtb::from_raw_parts_filtered(layout.p_to_v(dtb_addr) as _, |e| {
            matches!(e, Misaligned(4) | LastCompVersion(_))
        })
    };
    let mut max = 0;
    dtb.unwrap().walk(|path, obj| match obj {
        DtbObj::SubNode { name } => {
            if path.is_root() && name.starts_with("memory") {
                StepInto
            } else {
                StepOver
            }
        }
        DtbObj::Property(Property::Reg(reg)) if path.name().starts_with("memory") => {
            let p_start = layout.p_start();
            for segment in reg {
                unsafe {
                    let (ptr, size) = if segment.contains(&p_start) {
                        let addr = layout.boot_pt_root() + 4096;
                        (addr as *mut u8, layout.p_to_v(segment.end) - addr)
                    } else {
                        (layout.p_to_v(segment.start) as _, segment.len())
                    };
                    max = max.max(ptr as usize + size);
                    GLOBAL.transfer(NonNull::new_unchecked(ptr), size);
                };
            }
            StepOut
        }
        DtbObj::Property(_) => StepOver,
    });
    max
}
