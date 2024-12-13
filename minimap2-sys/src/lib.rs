#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

#[cfg(feature = "bindgen")]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(all(not(feature = "bindgen")))]
include!("bindings.rs");

unsafe impl Send for mm_idx_t {}
unsafe impl Send for mm_idx_reader_t {}
unsafe impl Send for mm_mapopt_t {}

use paste::paste;

impl Drop for mm_idx_t {
    fn drop(&mut self) {
        unsafe { mm_idx_destroy(self) };
    }
}

use std::mem::MaybeUninit;

impl Default for mm_mapopt_t {
    fn default() -> Self {
        unsafe {
            let mut opt = MaybeUninit::uninit();
            mm_mapopt_init(opt.as_mut_ptr());
            opt.assume_init()
        }
    }
}


macro_rules! add_flag_methods {
    ($ty:ty, $struct_name:ident, $(($set_name:ident, $unset_name:ident, $flag:expr)),+) => {
        impl $struct_name {
            $(
                paste! {
                    #[inline(always)]
                    #[doc = "Set the " $flag " flag"]
                    pub fn $set_name(&mut self) {
                        self.flag |= $flag as $ty;
                    }

                    #[inline(always)]
                    #[doc = "Unset the " $flag " flag"]
                    pub fn $unset_name(&mut self) {
                        self.flag &= !$flag as $ty;
                    }
                }
            )*
        }
    };
}

add_flag_methods!(
    i64,
    mm_mapopt_t,
    (set_no_dual, unset_no_dual, MM_F_NO_DUAL),
    (set_no_diag, unset_no_diag, MM_F_NO_DIAG),
    (set_cigar, unset_cigar, MM_F_CIGAR),
    (set_out_sam, unset_out_sam, MM_F_OUT_SAM),
    (set_no_qual, unset_no_qual, MM_F_NO_QUAL),
    (set_out_cg, unset_out_cg, MM_F_OUT_CG),
    (set_out_cs, unset_out_cs, MM_F_OUT_CS),
    (set_splice, unset_splice, MM_F_SPLICE),
    (set_splice_for, unset_splice_for, MM_F_SPLICE_FOR),
    (set_splice_rev, unset_splice_rev, MM_F_SPLICE_REV),
    (set_no_ljoin, unset_no_ljoin, MM_F_NO_LJOIN),
    (set_out_cs_long, unset_out_cs_long, MM_F_OUT_CS_LONG),
    (set_sr, unset_sr, MM_F_SR),
    (set_frag_mode, unset_frag_mode, MM_F_FRAG_MODE),
    (set_no_print_2nd, unset_no_print_2nd, MM_F_NO_PRINT_2ND),
    (set_two_io_threads, unset_two_io_threads, MM_F_2_IO_THREADS),
    (set_long_cigar, unset_long_cigar, MM_F_LONG_CIGAR),
    (set_indep_seg, unset_indep_seg, MM_F_INDEPEND_SEG),
    (set_splice_flank, unset_splice_flank, MM_F_SPLICE_FLANK),
    (set_softclip, unset_softclip, MM_F_SOFTCLIP),
    (set_for_only, unset_for_only, MM_F_FOR_ONLY),
    (set_rev_only, unset_rev_only, MM_F_REV_ONLY),
    (set_heap_sort, unset_heap_sort, MM_F_HEAP_SORT),
    (set_all_chains, unset_all_chains, MM_F_ALL_CHAINS),
    (set_out_md, unset_out_md, MM_F_OUT_MD),
    (set_copy_comment, unset_copy_comment, MM_F_COPY_COMMENT),
    (set_eqx, unset_eqx, MM_F_EQX),
    (set_paf_no_hit, unset_paf_no_hit, MM_F_PAF_NO_HIT),
    (set_no_end_flt, unset_no_end_flt, MM_F_NO_END_FLT),
    (set_hard_mlevel, unset_hard_mlevel, MM_F_HARD_MLEVEL),
    (set_sam_hit_only, unset_sam_hit_only, MM_F_SAM_HIT_ONLY),
    (set_rmq, unset_rmq, MM_F_RMQ),
    (set_qstrand, unset_qstrand, MM_F_QSTRAND),
    (set_no_inv, unset_no_inv, MM_F_NO_INV),
    (set_no_hash_name, unset_no_hash_name, MM_F_NO_HASH_NAME),
    (set_splice_old, unset_splice_old, MM_F_SPLICE_OLD),
    (set_secondary_seq, unset_secondary_seq, MM_F_SECONDARY_SEQ),
    (set_out_ds, unset_out_ds, MM_F_OUT_DS)
);

add_flag_methods!(
    std::os::raw::c_short,
    mm_idxopt_t,
    (set_hpc, unset_hpc, MM_I_HPC),
    (set_no_seq, unset_no_seq, MM_I_NO_SEQ),
    (set_no_name, unset_no_name, MM_I_NO_NAME)
);

impl Default for mm_idxopt_t {
    fn default() -> Self {
        unsafe {
            let mut opt = MaybeUninit::uninit();
            mm_idxopt_init(opt.as_mut_ptr());
            opt.assume_init()
        }
    }
}

// TODO: Add more tests!
#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::MaybeUninit;

    #[test]
    fn set_index_and_mapping_opts() {
        let mut mm_idxopt = MaybeUninit::uninit();
        let mut mm_mapopt = MaybeUninit::uninit();

        unsafe {
            mm_set_opt(
                std::ptr::null(),
                mm_idxopt.as_mut_ptr(),
                mm_mapopt.as_mut_ptr(),
            )
        };
        println!("{:#?}", unsafe { mm_idxopt.assume_init() });
        println!("{:#?}", unsafe { mm_mapopt.assume_init() }); // Run tests with --nocapture to see the output
    }

    #[test]
    fn mapopt() {
        let x: mm_mapopt_t = Default::default();
    }

    #[test]
    fn idxopt() {
        let x: mm_idxopt_t = Default::default();
    }

    #[test]
    fn test_mapopt_flags() {
        let mut opt = mm_mapopt_t::default();
        opt.set_no_qual();
        assert_eq!(opt.flag & MM_F_NO_QUAL as i64, MM_F_NO_QUAL as i64);

        opt.unset_no_qual();
        assert_eq!(opt.flag & MM_F_NO_QUAL as i64, 0_i64);
    }

    #[test]
    fn test_idxopt_flags() {
        let mut opt = mm_idxopt_t::default();
        opt.set_hpc();
        assert_eq!(opt.flag & MM_I_HPC as i16, MM_I_HPC as i16);

        opt.unset_hpc();
        assert_eq!(opt.flag & MM_I_HPC as i16, 0_i16);
    }
}
