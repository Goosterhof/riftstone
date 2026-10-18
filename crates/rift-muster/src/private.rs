//! A name that is the player's own.

/// A character's name from a save: the Arisen's, a pawn's. It prints as `‹private›` through
/// `Debug` and `Display`, and it is not `Serialize`, so it cannot slip into a log line, an error
/// or a cache by accident. Only [`Private::expose`] gives the text, for the window's own view.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Private(String);

impl Private {
    pub fn new(text: impl Into<String>) -> Self {
        Private(text.into())
    }

    /// The name itself, for the window's view and nothing else.
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for Private {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("‹private›")
    }
}

impl std::fmt::Display for Private {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("‹private›")
    }
}

#[cfg(test)]
mod tests {
    use super::Private;

    #[test]
    fn a_name_never_prints() {
        let name = Private::new("Odalric");
        assert_eq!(format!("{name}"), "‹private›");
        assert_eq!(format!("{name:?}"), "‹private›");
        assert_eq!(format!("{:?}", Some(name.clone())), "Some(‹private›)");
        assert_eq!(name.expose(), "Odalric");
    }
}
