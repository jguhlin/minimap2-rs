use pyo3::prelude::*;

// Reference: https://github.com/pola-rs/pyo3-polars

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
