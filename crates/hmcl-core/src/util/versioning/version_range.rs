//! Version ranges with inclusive bounds.
//!
//! Port of `org.jackhuang.hmcl.util.versioning.VersionRange`.

use std::fmt;

use super::version_number::VersionNumber;

/// A possibly-empty inclusive range of versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionRange {
    pub(crate) minimum: Option<VersionNumber>,
    pub(crate) maximum: Option<VersionNumber>,
    pub(crate) empty: bool,
}

impl VersionRange {
    /// The empty range containing no versions.
    pub fn empty() -> Self {
        Self {
            minimum: None,
            maximum: None,
            empty: true,
        }
    }

    /// The range containing all versions.
    pub fn all() -> Self {
        Self {
            minimum: None,
            maximum: None,
            empty: false,
        }
    }

    pub fn between(minimum: VersionNumber, maximum: VersionNumber) -> Self {
        assert!(minimum <= maximum);
        Self {
            minimum: Some(minimum),
            maximum: Some(maximum),
            empty: false,
        }
    }

    pub fn at_least(minimum: VersionNumber) -> Self {
        Self {
            minimum: Some(minimum),
            maximum: None,
            empty: false,
        }
    }

    pub fn at_most(maximum: VersionNumber) -> Self {
        Self {
            minimum: None,
            maximum: Some(maximum),
            empty: false,
        }
    }

    pub fn is_(version: VersionNumber) -> Self {
        Self {
            minimum: Some(version.clone()),
            maximum: Some(version),
            empty: false,
        }
    }

    pub fn get_minimum(&self) -> Option<&VersionNumber> {
        self.minimum.as_ref()
    }

    pub fn get_maximum(&self) -> Option<&VersionNumber> {
        self.maximum.as_ref()
    }

    pub fn is_empty(&self) -> bool {
        self.empty
    }

    pub fn is_all(&self) -> bool {
        !self.empty && self.minimum.is_none() && self.maximum.is_none()
    }

    pub fn contains(&self, version: &VersionNumber) -> bool {
        if self.empty {
            return false;
        }
        if self.is_all() {
            return true;
        }
        (self.minimum.as_ref().is_none_or(|min| min <= version))
            && (self.maximum.as_ref().is_none_or(|max| max >= version))
    }

    pub fn is_overlapped_by(&self, that: &VersionRange) -> bool {
        if self.empty || that.empty {
            return false;
        }
        if self.is_all() || that.is_all() {
            return true;
        }
        match (&self.minimum, &self.maximum) {
            (None, Some(max)) => that.minimum.as_ref().is_none_or(|min| min <= max),
            (Some(min), None) => that.maximum.as_ref().is_none_or(|max| max >= min),
            (Some(min), Some(max)) => {
                that.contains(min)
                    || that.contains(max)
                    || that.minimum.as_ref().is_some_and(|m| self.contains(m))
            }
            (None, None) => true,
        }
    }

    pub fn intersection_with(&self, that: &VersionRange) -> VersionRange {
        if self.is_all() {
            return that.clone();
        }
        if that.is_all() {
            return self.clone();
        }
        if !self.is_overlapped_by(that) {
            return VersionRange::empty();
        }

        let new_minimum = match (&self.minimum, &that.minimum) {
            (None, other) => other.clone(),
            (other, None) => other.clone(),
            (Some(a), Some(b)) => Some(if a >= b { a.clone() } else { b.clone() }),
        };
        let new_maximum = match (&self.maximum, &that.maximum) {
            (None, other) => other.clone(),
            (other, None) => other.clone(),
            (Some(a), Some(b)) => Some(if a <= b { a.clone() } else { b.clone() }),
        };

        VersionRange {
            minimum: new_minimum,
            maximum: new_maximum,
            empty: false,
        }
    }
}

impl fmt::Display for VersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return f.write_str("EMPTY");
        }
        if self.is_all() {
            return f.write_str("ALL");
        }
        match (&self.minimum, &self.maximum) {
            (None, Some(max)) => write!(f, "At most {max}"),
            (Some(min), None) => write!(f, "At least {min}"),
            (Some(min), Some(max)) => write!(f, "[{min}..{max}]"),
            (None, None) => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(spec: VersionRange) -> VersionRange {
        spec
    }

    #[test]
    fn test_contains() {
        let all = VersionRange::all();
        assert!(all.contains(&VersionNumber::as_version("1.0")));

        let between = range(VersionRange::between(
            VersionNumber::as_version("1.8"),
            VersionNumber::as_version("1.21"),
        ));
        assert!(between.contains(&VersionNumber::as_version("1.8")));
        assert!(between.contains(&VersionNumber::as_version("1.20.1")));
        assert!(between.contains(&VersionNumber::as_version("1.21")));
        assert!(!between.contains(&VersionNumber::as_version("1.7.10")));
        assert!(!between.contains(&VersionNumber::as_version("1.21.1")));

        assert!(!VersionRange::empty().contains(&VersionNumber::as_version("1.0")));
    }

    #[test]
    fn test_overlap_and_intersection() {
        let a = VersionRange::between(
            VersionNumber::as_version("1.16"),
            VersionNumber::as_version("1.20"),
        );
        let b = VersionRange::between(
            VersionNumber::as_version("1.18"),
            VersionNumber::as_version("1.22"),
        );
        assert!(a.is_overlapped_by(&b));
        let inter = a.intersection_with(&b);
        assert_eq!(
            inter.get_minimum().unwrap(),
            &VersionNumber::as_version("1.18")
        );
        assert_eq!(
            inter.get_maximum().unwrap(),
            &VersionNumber::as_version("1.20")
        );

        let c = VersionRange::between(
            VersionNumber::as_version("1.8"),
            VersionNumber::as_version("1.12"),
        );
        assert!(!a.is_overlapped_by(&c));
        assert!(a.intersection_with(&c).is_empty());
    }
}
