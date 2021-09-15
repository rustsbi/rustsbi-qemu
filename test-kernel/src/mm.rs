use buddy_system_allocator::LockedHeap;

const HEAP_SIZE: usize = 64 * 1024; // 64KiB
#[link_section = ".bss.uninit"]
static mut HEAP_SPACE: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
#[global_allocator]
static HEAP: LockedHeap<32> = LockedHeap::empty();

pub fn init_heap() {
    unsafe { HEAP.lock().init(HEAP_SPACE.as_ptr() as usize, HEAP_SIZE) }
}
