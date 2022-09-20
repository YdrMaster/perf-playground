use core::ptr::NonNull;
use customizable_buddy::{BuddyAllocator, LinkedListBuddy, UsizeBuddy};
use page_table::{MmuMeta, Sv39};

/// 全局页帧分配器。
pub static mut GLOBAL: BuddyAllocator<20, UsizeBuddy, LinkedListBuddy> = BuddyAllocator::new();

/// 建立页分配器。
pub(crate) fn init_global(start: usize, offset: usize, end: usize, dtb_addr: usize) -> usize {
    use dtb_walker::{Dtb, DtbObj, HeaderError::*, Property, WalkOperation::*};
    unsafe { GLOBAL.init(Sv39::PAGE_BITS, NonNull::new_unchecked(start as *mut u8)) };
    // 从设备树解析内存信息
    let dtb = unsafe {
        Dtb::from_raw_parts_filtered((dtb_addr + offset) as _, |e| {
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
            for segment in reg {
                unsafe {
                    let (ptr, size) = if segment.contains(&(start - offset)) {
                        const ALIGN: usize = 4096 - 1;
                        let addr = (end + crate::STACK_SIZE + ALIGN + 4096) & !ALIGN;
                        let size = segment.end + offset - addr;
                        (addr as *mut u8, size)
                    } else {
                        (segment.start as _, segment.len())
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
