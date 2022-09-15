use core::ptr::NonNull;
use customizable_buddy::{BuddyAllocator, LinkedListBuddy, UsizeBuddy};
use linker::MemInfo;
use page_table::{MmuMeta, Sv39};

/// 全局页帧分配器。
pub static mut GLOBAL: BuddyAllocator<20, UsizeBuddy, LinkedListBuddy> = BuddyAllocator::new();

/// 建立页分配器。
pub(crate) fn init_global(info: MemInfo, dtb_addr: usize) {
    use dtb_walker::{Dtb, DtbObj, HeaderError::*, Property, WalkOperation::*};
    unsafe {
        GLOBAL.init(
            Sv39::PAGE_BITS,
            NonNull::new_unchecked(info.base as *mut u8),
        )
    };
    // 从设备树解析内存信息
    unsafe {
        Dtb::from_raw_parts_filtered((dtb_addr + info.offset) as _, |e| {
            matches!(e, Misaligned(4) | LastCompVersion(_))
        })
    }
    .unwrap()
    .walk(|path, obj| match obj {
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
                    let (ptr, size) = if segment.contains(&info.base) {
                        let stack_top = crate::boot_stack().as_mut_ptr_range().end;
                        let size = segment.end + info.offset - stack_top as usize;
                        (stack_top, size)
                    } else {
                        (segment.start as _, segment.len())
                    };
                    GLOBAL.transfer(NonNull::new_unchecked(ptr), size)
                };
            }
            StepOut
        }
        DtbObj::Property(_) => StepOver,
    });
}
