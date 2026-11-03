fn main() {
    if cfg!(test_only_flag) {
        println!("testExtraRustcOpts leaked into the normal build");
    } else {
        println!("Hello, {}!", dep::name());
    }
}

#[cfg(test)]
mod tests {
    // Only compiled if testExtraRustcOpts reached the test build.
    #[cfg(test_only_flag)]
    #[test]
    fn test_extra_rustc_opts_applied() {}
}
