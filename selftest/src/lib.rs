//! A tiny crate for `.github/workflows/selftest.yml`, which runs the reusable
//! `rust-ci.yml` on it.

pub fn hello() -> &'static str {
    "hello"
}

#[cfg(feature = "extra")]
pub fn extra() -> bool {
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn says_hello() {
        assert_eq!(super::hello(), "hello");
    }

    #[cfg(feature = "extra")]
    #[test]
    fn extra_feature_is_on() {
        assert!(super::extra());
    }
}
