//! The greeting this workspace exists to print.

/// Returns the greeting.
#[must_use]
pub fn hello() -> &'static str {
    "Hello, world!"
}

#[cfg(test)]
mod tests {
    use super::hello;

    #[test]
    fn greets() {
        assert_eq!(hello(), "Hello, world!");
    }
}
