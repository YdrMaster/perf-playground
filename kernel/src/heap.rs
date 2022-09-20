use crate::page::GLOBAL;
use alloc::alloc::handle_alloc_error;
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
};
use customizable_buddy::{BuddyAllocator, LinkedListBuddy, UsizeBuddy};

/// 全局页帧分配器。
static mut HEAP: BuddyAllocator<20, UsizeBuddy, LinkedListBuddy> = BuddyAllocator::new();

struct Heap;

#[global_allocator]
static _HEAP: Heap = Heap;

/// 建立页分配器。
pub(crate) fn init_heap(start: usize) {
    unsafe { HEAP.init(3, NonNull::new_unchecked(start as *mut u8)) };
}

unsafe impl GlobalAlloc for Heap {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if let Ok((ptr, _)) = HEAP.allocate_layout::<u8>(layout) {
            ptr.as_ptr()
        } else if let Ok((ptr, size)) = GLOBAL.allocate_layout::<u8>(
            Layout::from_size_align_unchecked(layout.size().next_power_of_two(), layout.align()),
        ) {
            HEAP.transfer(ptr, size);
            HEAP.allocate_layout::<u8>(layout).unwrap().0.as_ptr()
        } else {
            handle_alloc_error(layout)
        }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        HEAP.deallocate_layout(NonNull::new(ptr).unwrap(), layout)
    }
}
