#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

#[cfg(feature = "bindgen")]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(not(feature = "bindgen"))]
pub mod bindings;

#[cfg(not(feature = "bindgen"))]
pub use bindings::*;

pub static PRESET0: i8 = 0;
/*

impl Default for mm_idxopt_t {
    fn default() -> Self {
        mm_idxopt_t {
            k: 15,
            w: 10,
            flag: 0,
            bucket_bits: 14,
            mini_batch_size: 50000000,
            batch_size: 4000000000,
        }
    }
}

impl Default for mm_mapopt_t {
    fn default() -> Self {
        mm_mapopt_t {
            seed: 11,
            mid_occ_frac: 2e-4,
            min_mid_occ: 10,
            max_mid_occ: 1_000_000,
            sdust_thres: 0, // no SDUST masking
            q_occ_frac: 0.01,
            min_cnt: 3,
            min_chain_score: 40,
            bw: 500,
            bw_long: 20_000,
            max_gap: 5_000,
            max_gap_ref: -1,
            max_chain_skip: 25,
            max_chain_iter: 5_000,
            rmq_inner_dist: 1000,
            rmq_size_cap: 100000,
            rmq_rescue_size: 1000,
            rmq_rescue_ratio: 0.1,
            chain_gap_scale: 0.8,
            chain_skip_scale: 0.0,
            max_max_occ: 4095,
            occ_dist: 500,
            mask_level: 0.5,
            mask_len: libc::INT_MAX,
            pri_ratio: 0.8,
            best_n: 5,
            alt_drop: 0.15,
            a: 2,
            b: 4,
            q: 4,
            e: 2,
            q2: 24,
            e2: 1,
            sc_ambi: 1,
            zdrop: 400,
            zdrop_inv: 200,
            end_bonus: -1,
            min_dp_max: 40 * 2, // min_chain_score * a
            min_ksw_len: 200,
            anchor_ext_len: 20,
            anchor_ext_shift: 6,
            max_clip_ratio: 1.0,
            mini_batch_size: 500000000,
            max_sw_mat: 100000000,
            cap_kalloc: 1000000000,
            rank_min_len: 500,
            rank_frac: 0.9,
            pe_ori: 0,
            pe_bonus: 33,

            // These values just grabbed randomly from the file...
            flag: 0,
            junc_bonus: 9, 
            max_frag_len: 800,
            max_qlen: 0,
            noncan: 9,
            mid_occ: todo!(),
            max_occ: todo!(),
            split_prefix: todo!(),
            

        }
    }
} */


#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::MaybeUninit;
    use libc::{c_char};

    #[test]
    fn does_it_work() {
        let mut mm_idxopt = MaybeUninit::uninit();
        let mut mm_mapopt = MaybeUninit::uninit();

        unsafe { mm_set_opt(&PRESET0, mm_idxopt.as_mut_ptr(), mm_mapopt.as_mut_ptr()) };
        println!("{:#?}", unsafe { mm_idxopt.assume_init() });
        println!("{:#?}", unsafe { mm_mapopt.assume_init() }); // Run tests with --nocapture to see the output
        
    }
}
