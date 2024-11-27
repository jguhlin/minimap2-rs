#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

#[cfg(feature = "bindgen")]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(all(not(feature = "bindgen"), not(feature = "rust-threads")))]
include!("bindings.rs");

#[cfg(feature = "rust-threads")]
include!("bindings_rust_threads.rs");

unsafe impl Send for mm_idx_t {}
unsafe impl Send for mm_idx_reader_t {}
unsafe impl Send for mm_mapopt_t {}

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

impl Default for mm_idxopt_t {
    fn default() -> Self {
        unsafe {
            let mut opt = MaybeUninit::uninit();
            mm_idxopt_init(opt.as_mut_ptr());
            opt.assume_init()
        }
    }
}

#[cfg(feature = "rust-threads")]
mod rust_threads_index;

#[cfg(feature = "rust-threads")]
pub use rust_threads_index::*;

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
}
