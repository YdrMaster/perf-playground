use crate::{non_null, LAYOUT};
use core::{fmt, ptr::NonNull};
use page_table::{PageTable, PageTableFormatter, Pte, VAddr, VmFlags, VmMeta, PPN, VPN};
use rangemap::RangeSet;

pub(crate) struct AddressSpace<Meta: VmMeta, M: PageManager<Meta>> {
    segments: RangeSet<VPN<Meta>>,
    root: NonNull<Pte<Meta>>,
    manager: M,
}

impl<Meta: VmMeta, M: PageManager<Meta>> AddressSpace<Meta, M> {
    pub fn new(mut manager: M) -> Self {
        let ppn = manager.allocate(VmFlags::VALID, 1).ppn();
        Self {
            segments: RangeSet::new(),
            root: manager.p_to_v(ppn),
            manager,
        }
    }

    pub fn root_ppn(&self) -> PPN<Meta> {
        self.manager.v_to_p(self.root)
    }

    pub fn kernel(&mut self, flags: VmFlags<Meta>) {
        let info = unsafe { &LAYOUT };
        let top_entries = 1 << Meta::LEVEL_BITS.last().unwrap();
        let ppn_bits = Meta::pages_in_table(Meta::MAX_LEVEL - 1).trailing_zeros();
        // 内核线性段
        self.segments.insert(
            VAddr::<Meta>::new(info.offset()).floor()..VAddr::<Meta>::new(info.top()).ceil(),
        );
        // 页表
        unsafe { core::slice::from_raw_parts_mut(self.root.as_ptr(), top_entries) }
            .iter_mut()
            .skip(
                VAddr::<Meta>::new(info.offset())
                    .floor()
                    .index_in(Meta::MAX_LEVEL),
            )
            .take(
                VAddr::<Meta>::new(info.v_to_p(info.top()))
                    .ceil()
                    .ceil(Meta::MAX_LEVEL),
            )
            .enumerate()
            .for_each(|(i, pte)| *pte = flags.build_pte(PPN::new(i << ppn_bits)));
    }
}

impl<Meta: VmMeta, M: PageManager<Meta>> fmt::Debug for AddressSpace<Meta, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for seg in self.segments.iter() {
            writeln!(
                f,
                "{:#x}..{:#x}",
                seg.start.base().val(),
                seg.end.base().val()
            )?;
        }
        writeln!(
            f,
            "{:?}",
            PageTableFormatter {
                pt: unsafe { PageTable::from_root(self.root) },
                f: |ppn| non_null(VPN::<Meta>::new(ppn.val()).base().val())
            }
        )
    }
}

pub trait PageManager<Meta: VmMeta> {
    fn allocate(&mut self, flags: VmFlags<Meta>, len: usize) -> Pte<Meta>;
    fn deallocate(&mut self, pte: Pte<Meta>, len: usize);
    fn share(&mut self, pte: Pte<Meta>, len: usize) -> (Pte<Meta>, Pte<Meta>);
    fn exclude(&mut self, pte: Pte<Meta>, len: usize) -> Pte<Meta>;
    fn p_to_v<T>(&self, ppn: PPN<Meta>) -> NonNull<T>;
    fn v_to_p<T>(&self, ptr: NonNull<T>) -> PPN<Meta>;
}
