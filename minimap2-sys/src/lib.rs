#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

// #[cfg(feature = "bindgen")]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use std::error::Error;
use std::mem::MaybeUninit;
use std::path::Path;

// TODO: Add more tests!
#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::MaybeUninit;

    #[test]
    fn set_index_and_mapping_opts() {
        let mut mm_idxopt = MaybeUninit::uninit();
        let mut mm_mapopt = MaybeUninit::uninit();

        unsafe { mm_set_opt(&0, mm_idxopt.as_mut_ptr(), mm_mapopt.as_mut_ptr()) };
        println!("{:#?}", unsafe { mm_idxopt.assume_init() });
        println!("{:#?}", unsafe { mm_mapopt.assume_init() }); // Run tests with --nocapture to see the output
    }
}
