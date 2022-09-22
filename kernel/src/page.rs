use crate::{layout::KernelLayout, non_null};
use customizable_buddy::{BuddyAllocator, LinkedListBuddy, UsizeBuddy};
use page_table::{MmuMeta, Sv39};

/// 全局页帧分配器。
pub static mut GLOBAL: BuddyAllocator<20, UsizeBuddy, LinkedListBuddy> = BuddyAllocator::new();

/// 建立页分配器。
///
/// 返回线性地址空间的结束位置。
pub(crate) fn init_global(layout: &KernelLayout, dtb_addr: usize) -> usize {
    use dtb_walker::{Dtb, DtbObj, HeaderError::*, Property, WalkOperation::*};
    unsafe { GLOBAL.init(Sv39::PAGE_BITS, non_null::<u8>(layout.start())) };
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
            let p_start = layout.v_to_p(layout.start());
            for segment in reg {
                unsafe {
                    let (ptr, size) = if segment.contains(&p_start) {
                        // 从启动页表之后由页帧分配器管理
                        let addr = layout.boot_pt_root() + 4096;
                        (addr, layout.p_to_v(segment.end) - addr)
                    } else {
                        (layout.p_to_v(segment.start), segment.len())
                    };
                    max = max.max(ptr + size);
                    GLOBAL.transfer(non_null::<u8>(ptr), size);
                };
            }
            StepOut
        }
        DtbObj::Property(_) => StepOver,
    });
    max
}
