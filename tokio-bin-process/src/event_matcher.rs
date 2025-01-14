//! Structures for matching an [`Event`]

use std::fmt::Display;

use crate::event::{Event, Level};
use itertools::Itertools;
use regex_lite::Regex;

/// Use to check for any matching [`Event`]'s among a list of events.
#[derive(Debug)]
pub struct Events {
    pub events: Vec<Event>,
}

impl Events {
    pub fn assert_contains(&self, matcher: &EventMatcher) {
        if !self.events.iter().any(|e| matcher.matches(e)) {
            panic!(
                "An event with {matcher:?} was not found in the list of events:\n{}",
                self.events.iter().join("\n")
            )
        }
    }

    #[allow(dead_code)]
    fn assert_contains_in_order(&self, _matchers: &[EventMatcher]) {
        todo!()
    }
}

impl Display for Events {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for event in &self.events {
            if first {
                write!(f, "{event}")?;
            } else {
                write!(f, "\n{event}")?;
            }
            first = false;
        }

        Ok(())
    }
}

/// Use to check if an [`Event`] matches certain criteria.
///
/// Construct by chaining methods, e.g:
/// ```rust,no_run
/// # use tokio_bin_process::event_matcher::EventMatcher;
/// # use tokio_bin_process::event::Level;
/// # let event = todo!();
///
/// assert!(
///     EventMatcher::new()
///         .with_level(Level::Info)
///         .with_target("module::internal_module")
///         .with_message("Some message")
///         .matches(event)
/// );
/// ```
#[derive(Default, Debug)]
pub struct EventMatcher {
    level: Matcher<Level>,
    message: Matcher<String>,
    message_regex: RegexMatcher,
    target: Matcher<String>,
    pub(crate) count: Count,
}

impl EventMatcher {
    /// Creates a new [`EventMatcher`] that by default matches any Event.
    pub fn new() -> EventMatcher {
        EventMatcher::default()
    }

    /// Sets the matcher to only match an [`Event`] when it has the exact provided level
    pub fn with_level(mut self, level: Level) -> EventMatcher {
        self.level = Matcher::Matches(level);
        self
    }

    /// Sets the matcher to only match an [`Event`] when it has the exact provided target
    pub fn with_target(mut self, target: &str) -> EventMatcher {
        self.target = Matcher::Matches(target.to_owned());
        self
    }

    /// Sets the matcher to only match an [`Event`] when it has the exact provided message
    pub fn with_message(mut self, message: &str) -> EventMatcher {
        self.message = Matcher::Matches(message.to_owned());
        self
    }

    /// Sets the matcher to only match an [`Event`] when it matches the provided regex pattern
    pub fn with_message_regex(mut self, regex_pattern: &str) -> EventMatcher {
        self.message_regex = RegexMatcher::new(regex_pattern);
        self
    }

    /// Defines how many times the matcher must match to pass an assertion
    ///
    /// This is not used internally i.e. it has no effect on [`EventMatcher::matches`]
    /// Instead its only used by higher level assertion logic.
    pub fn with_count(mut self, count: Count) -> EventMatcher {
        self.count = count;
        self
    }

    /// Returns true only if this matcher matches the passed [`Event`]
    pub fn matches(&self, event: &Event) -> bool {
        self.level.matches(&event.level)
            && self.message.matches(&event.fields.message)
            && self.message_regex.matches(&event.fields.message)
            && self.target.matches(&event.target)
    }
}

/// Defines how many times the [`EventMatcher`] must match to pass an assertion.
#[derive(Debug)]
#[non_exhaustive]
pub enum Count {
    /// This matcher must match this many times to pass an assertion.
    Times(usize),
    /// This matcher may match 0 or more times and will still pass an assertion.
    /// Use sparingly but useful for ignoring a warning or error that is not appearing deterministically.
    Any,
    /// This matcher must match this many times or more to pass an asssertion
    GreaterThanOrEqual(usize),
    /// This matcher must match this many times or fewer to pass an asssertion
    LessThanOrEqual(usize),
}

impl From<usize> for Count {
    fn from(item: usize) -> Self {
        Self::Times(item)
    }
}

impl Default for Count {
    fn default() -> Self {
        Count::Times(1)
    }
}

#[derive(Debug, Default)]
enum Matcher<T: PartialEq> {
    Matches(T),
    #[default]
    Any,
}

impl<T: PartialEq> Matcher<T> {
    fn matches(&self, value: &T) -> bool {
        match self {
            Matcher::Matches(x) => value == x,
            Matcher::Any => true,
        }
    }
}

#[derive(Debug, Default)]
enum RegexMatcher {
    Matches(Regex),
    #[default]
    Any,
}

impl RegexMatcher {
    fn new(pattern: &str) -> Self {
        RegexMatcher::Matches(regex_lite::Regex::new(pattern).unwrap())
    }
}

impl RegexMatcher {
    fn matches(&self, value: &str) -> bool {
        match self {
            RegexMatcher::Matches(regex) => regex.is_match(value),
            RegexMatcher::Any => true,
        }
    }
}
