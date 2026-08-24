//! Version number comparison.
//!
//! Faithful port of Maven's `ComparableVersion` algorithm (Apache License 2.0),
//! which HMCL uses via `org.jackhuang.hmcl.util.versioning.VersionNumber`.
//! See <https://maven.apache.org/pom.html#Version_Order_Specification>.
//!
//! Sub-lists are stored in an arena indexed by `Item::List(usize)`, mirroring
//! the object aliasing used in the Java implementation.

use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Clone)]
enum Item {
    /// Numeric item fitting in an `i64` (at most 18 significant digits).
    Long(i64),
    /// Numeric item longer than 18 significant digits.
    BigInteger(String),
    /// Qualifier such as `alpha`, `beta`, `rc`, `sp` or arbitrary text.
    String(String, bool),
    /// Sublist introduced by a separator character; index into the arena.
    List(usize),
}

impl Item {
    fn is_null(&self, arena: &[ListItem]) -> bool {
        match self {
            Item::Long(value) => *value == 0,
            Item::BigInteger(_) => false,
            Item::String(value, _) => value.is_empty(),
            Item::List(index) => arena[*index].items.is_empty(),
        }
    }
}

fn append_list(list: &ListItem, buffer: &mut String, arena: &[ListItem]) {
    if let Some(separator) = list.separator {
        buffer.push(separator);
    }
    let init_len = buffer.len();
    for item in &list.items {
        if buffer.len() > init_len && !matches!(item, Item::List(_)) {
            buffer.push('.');
        }
        append_item(item, buffer, arena);
    }
}

fn append_item(item: &Item, buffer: &mut String, arena: &[ListItem]) {
    match item {
        Item::Long(value) => buffer.push_str(&value.to_string()),
        Item::BigInteger(value) => buffer.push_str(value),
        Item::String(value, _) => buffer.push_str(value),
        Item::List(index) => append_list(&arena[*index], buffer, arena),
    }
}

/// Compare two non-negative integer strings (no leading zeros) as numbers.
fn compare_bigint_strs(a: &str, b: &str) -> Ordering {
    match a.len().cmp(&b.len()) {
        Ordering::Equal => a.cmp(b),
        other => other,
    }
}

#[derive(Debug, Clone)]
struct ListItem {
    items: Vec<Item>,
    separator: Option<char>,
}

const MAX_LONGITEM_LENGTH: usize = 18;
const SEPARATORS: &str = "!\"#$%&'()*+,-/:;<=>?@[\\]^_`{|}~";
const PRE_PREFIXES: [&str; 5] = ["alpha", "beta", "pre", "rc", "experimental"];

/// Parses a single version component: a plain number, a long number or a qualifier.
fn parse_item(buf: &str) -> Item {
    let mut number_length = 0usize;
    let mut leading_zero = true;
    for ch in buf.chars() {
        if ch.is_ascii_digit() {
            if ch != '0' {
                leading_zero = false;
            }
            if !leading_zero {
                number_length += 1;
            }
        } else {
            let lower = buf.trim().to_lowercase();
            let pre = PRE_PREFIXES.iter().any(|prefix| lower.starts_with(prefix));
            return Item::String(buf.to_owned(), pre);
        }
    }

    if number_length == 0 {
        Item::Long(0)
    } else if number_length <= MAX_LONGITEM_LENGTH {
        Item::Long(buf.parse().expect("valid numeric buffer"))
    } else {
        let trimmed = buf.trim_start_matches('0').to_owned();
        Item::BigInteger(if trimmed.is_empty() {
            "0".to_owned()
        } else {
            trimmed
        })
    }
}

/// A comparable version number implementing the Maven version order specification.
///
/// Equality is based on the canonical form, so `1.0 == 1` and `1-0 == 1`.
#[derive(Debug, Clone)]
pub struct VersionNumber {
    value: String,
    arena: Vec<ListItem>,
    canonical: String,
}

impl VersionNumber {
    /// Parse `version` into a comparable representation.
    pub fn as_version(version: &str) -> Self {
        Self::parse(version)
    }

    /// Compare two version strings.
    pub fn compare(version1: &str, version2: &str) -> Ordering {
        Self::as_version(version1).cmp(&Self::as_version(version2))
    }

    /// Return the canonical form of `str`, normalizing `1.0` to `1` etc.
    pub fn normalize(str: &str) -> String {
        Self::parse(str).canonical
    }

    /// Whether `version` consists only of dot-separated decimal numbers
    /// with at most 9 digits each (i.e. storable as `int` in Java semantics).
    pub fn is_int_version_number(version: &str) -> bool {
        if version.is_empty() {
            return false;
        }
        let mut idx = 0usize;
        loop {
            let rest = &version[idx..];
            let dot_index = rest.find('.');
            if dot_index == Some(0) || dot_index == Some(rest.len() - 1) {
                return false;
            }
            let end_index = dot_index.unwrap_or(rest.len());
            if end_index > 9 {
                return false;
            }
            if !rest[..end_index].bytes().all(|b| b.is_ascii_digit()) {
                return false;
            }
            match dot_index {
                None => return true,
                Some(dot) => idx += dot + 1,
            }
        }
    }

    fn parse(version: &str) -> Self {
        let mut arena = vec![ListItem {
            items: Vec::new(),
            separator: None,
        }];
        let mut current = 0usize;

        let mut is_digit = false;
        let mut start_index = 0usize;

        let push_item = |arena: &mut Vec<ListItem>, current: usize, item: Item| {
            arena[current].items.push(item);
        };

        let chars: Vec<char> = version.chars().collect();
        for (i, c) in chars.iter().enumerate() {
            if *c == '.' {
                if i == start_index {
                    push_item(&mut arena, current, Item::Long(0));
                } else {
                    push_item(&mut arena, current, parse_item(&version[start_index..i]));
                }
                start_index = i + 1;
            } else if SEPARATORS.contains(*c) {
                if i == start_index {
                    push_item(&mut arena, current, Item::Long(0));
                } else {
                    push_item(&mut arena, current, parse_item(&version[start_index..i]));
                }
                start_index = i + 1;

                let sub = arena.len();
                arena.push(ListItem {
                    items: Vec::new(),
                    separator: Some(*c),
                });
                push_item(&mut arena, current, Item::List(sub));
                current = sub;
            } else if c.is_ascii_digit() {
                if !is_digit && i > start_index {
                    push_item(&mut arena, current, parse_item(&version[start_index..i]));
                    start_index = i;

                    let sub = arena.len();
                    arena.push(ListItem {
                        items: Vec::new(),
                        separator: None,
                    });
                    push_item(&mut arena, current, Item::List(sub));
                    current = sub;
                }
                is_digit = true;
            } else {
                if is_digit && i > start_index {
                    push_item(&mut arena, current, parse_item(&version[start_index..i]));
                    start_index = i;

                    let sub = arena.len();
                    arena.push(ListItem {
                        items: Vec::new(),
                        separator: None,
                    });
                    push_item(&mut arena, current, Item::List(sub));
                    current = sub;
                }
                is_digit = false;
            }
        }

        if version.len() > start_index {
            push_item(&mut arena, current, parse_item(&version[start_index..]));
        }

        for i in (0..arena.len()).rev() {
            let mut j = arena[i].items.len();
            loop {
                if j == 0 {
                    break;
                }
                let last = &arena[i].items[j - 1];
                let is_list = matches!(last, Item::List(_));
                let is_null = last.is_null(&arena);
                if is_null {
                    arena[i].items.remove(j - 1);
                } else if is_list {
                    // Non-null sublist: keep it, continue scanning earlier items.
                } else {
                    break;
                }
                j -= 1;
            }
        }

        let canonical = {
            let mut buffer = String::new();
            append_list(&arena[0], &mut buffer, &arena);
            buffer
        };

        Self {
            value: version.to_owned(),
            arena,
            canonical,
        }
    }

    /// The original version string.
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// The canonical form of this version, used for equality.
    pub fn get_canonical(&self) -> &str {
        &self.canonical
    }
}

impl Ord for VersionNumber {
    fn cmp(&self, other: &Self) -> Ordering {
        cross_list_compare(0, &self.arena, 0, &other.arena)
    }
}

/// Compare the root lists of two versions living in different arenas.
fn cross_list_compare(
    a_index: usize,
    arena_a: &[ListItem],
    b_index: usize,
    arena_b: &[ListItem],
) -> Ordering {
    let mut left = arena_a[a_index].items.iter().peekable();
    let mut right = arena_b[b_index].items.iter().peekable();
    loop {
        match (left.next(), right.next()) {
            (Some(l), Some(r)) => {
                let result = cross_item_compare(l, Some(r), arena_a, arena_b);
                if result != Ordering::Equal {
                    return result;
                }
            }
            (None, Some(r)) => {
                let result = cross_item_compare(r, None, arena_b, arena_a).reverse();
                if result != Ordering::Equal {
                    return result;
                }
            }
            (Some(l), None) => {
                let result = cross_item_compare(l, None, arena_a, arena_b);
                if result != Ordering::Equal {
                    return result;
                }
            }
            (None, None) => return Ordering::Equal,
        }
    }
}

/// Compare items from two different arenas.
fn cross_item_compare(
    a: &Item,
    b: Option<&Item>,
    arena_a: &[ListItem],
    arena_b: &[ListItem],
) -> Ordering {
    match b {
        None => match a {
            Item::Long(value) => {
                if *value == 0 {
                    Ordering::Equal
                } else {
                    Ordering::Greater
                }
            }
            Item::BigInteger(_) => Ordering::Greater,
            Item::String(_, pre) => {
                if *pre {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            }
            Item::List(index) => {
                let list = &arena_a[*index];
                if list.items.is_empty() {
                    Ordering::Equal
                } else {
                    cross_item_compare(&list.items[0], None, arena_a, arena_b)
                }
            }
        },
        Some(other) => match (a, other) {
            (Item::Long(x), Item::Long(y)) => x.cmp(y),
            (Item::Long(_), Item::BigInteger(_)) => Ordering::Less,
            (Item::Long(_), Item::String(_, _)) => Ordering::Greater,
            (Item::Long(_), Item::List(_)) => Ordering::Greater,
            (Item::BigInteger(_), Item::Long(_)) => Ordering::Greater,
            (Item::BigInteger(x), Item::BigInteger(y)) => compare_bigint_strs(x, y),
            (Item::BigInteger(_), Item::String(_, _)) => Ordering::Greater,
            (Item::BigInteger(_), Item::List(_)) => Ordering::Greater,
            (Item::String(_, _), Item::Long(_)) => Ordering::Less,
            (Item::String(_, _), Item::BigInteger(_)) => Ordering::Less,
            (Item::String(x, _), Item::String(y, _)) => x.cmp(y),
            (Item::String(_, _), Item::List(_)) => Ordering::Less,
            (Item::List(_), Item::Long(_)) => Ordering::Less,
            (Item::List(_), Item::BigInteger(_)) => Ordering::Less,
            (Item::List(_), Item::String(_, _)) => Ordering::Greater,
            (Item::List(x), Item::List(y)) => cross_list_compare(*x, arena_a, *y, arena_b),
        },
    }
}

impl PartialOrd for VersionNumber {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for VersionNumber {
    fn eq(&self, other: &Self) -> bool {
        self.canonical == other.canonical
    }
}

impl Eq for VersionNumber {}

impl std::hash::Hash for VersionNumber {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.canonical.hash(state);
    }
}

impl fmt::Display for VersionNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_less_than(s1: &str, s2: &str) {
        assert!(
            VersionNumber::compare(s1, s2) == Ordering::Less,
            "{s1} should be less than {s2}"
        );
        assert!(
            VersionNumber::compare(s2, s1) == Ordering::Greater,
            "{s2} should be greater than {s1}"
        );
    }

    #[test]
    fn test_canonical() {
        assert_eq!(VersionNumber::normalize("3.2.0.0"), "3.2");
        assert_eq!(VersionNumber::normalize("3.2.0.0-5"), "3.2-5");
        assert_eq!(VersionNumber::normalize("3.2.0.0-0"), "3.2");
        assert_eq!(VersionNumber::normalize("3.2--------"), "3.2");
        assert_eq!(VersionNumber::normalize("3.0002"), "3.2");
        assert_eq!(
            VersionNumber::normalize("1.7.2$%%^@&snapshot-3.1.1"),
            "1.7.2$%%^@&snapshot-3.1.1"
        );
        assert_eq!(
            VersionNumber::normalize("1.99999999999999999999"),
            "1.99999999999999999999"
        );
        assert_eq!(
            VersionNumber::normalize("1.0099999999999999999999"),
            "1.99999999999999999999"
        );
        assert_eq!(
            VersionNumber::normalize("1.99999999999999999999.0"),
            "1.99999999999999999999"
        );
        assert_eq!(
            VersionNumber::normalize("1.99999999999999999999--------"),
            "1.99999999999999999999"
        );
    }

    #[test]
    fn test_is_int_version() {
        for version in [
            "", " ", ".", "1.", ".1", ".1.", "1..8", "1.8.", ".1.8", "1.7.10forge1614_FTBInfinity",
            "3.2-5", "1.9999999999",
        ] {
            assert!(!VersionNumber::is_int_version_number(version), "{version:?}");
        }
        for version in [
            "0", "1", "0.1", "0.1.0", "1.8", "1.12.2", "1.13.1", "1.999999999", "999999999.0",
        ] {
            assert!(VersionNumber::is_int_version_number(version), "{version:?}");
        }
    }

    #[test]
    fn test_comparator() {
        assert_less_than("1.7.10forge1614_FTBInfinity", "1.12.2");
        assert_less_than("1.8.0_51", "1.8.0.51");
        assert_less_than("1.8.0_77", "1.8.0_151");
        assert_less_than("1.6.0_22", "1.8.0_11");
        assert_less_than("1.7.0_22", "1.7.99");
        assert_less_than("1.12.2-14.23.4.2739", "1.12.2-14.23.5.2760");
        assert_less_than("1.9", "1.99999999999999999999");
        assert_less_than("1.99999999999999999999", "1.199999999999999999999");
        assert_less_than("1.99999999999999999999", "2");
        assert_less_than("1.99999999999999999999", "2.0");
        assert_less_than("1.0", "1.0-zzz");
        assert_less_than("1.0-beta.1", "1.0");
        assert_less_than("1.0-alpha.1", "1.0-beta.1");
        assert_less_than("3.6.15", "3.6.15.289");
        assert_less_than("3.6.15.289", "3.6.16");
    }

    #[test]
    fn test_sorting() {
        // Port of VersionNumberTest.testSorting: the input list is already
        // sorted under `compare then String::compareTo`.
        let input = [
            "0",
            "0.10.0",
            "1.6.4",
            "1.6.4-Forge9.11.1.1345",
            "1.7.10",
            "1.7.10Agrarian_Skies_2",
            "1.7.10-F1614-L",
            "1.7.10-FL1614_04",
            "1.7.10-Forge10.13.4.1614-1.7.10",
            "1.7.10-Forge1614",
            "1.7.10Forge1614_FTBInfinity-2.6.0",
            "1.7.10Forge1614_FTBInfinity-3.0.1",
            "1.7.10-Forge1614.1",
            "1.7.10forge1614_ATlauncher",
            "1.7.10forge1614_FTBInfinity",
            "1.7.10forge1614_FTBInfinity_server",
            "1.7.10forge1614test",
            "1.7.10-1614",
            "1.7.10-1614-test",
            "1.8",
            "1.8-forge1577",
            "1.8.9",
            "1.8.9-forge1902",
            "1.9",
            "1.10-alpha.2",
            "1.10-beta.1",
            "1.10-beta.2",
            "1.10",
            "1.10.2",
            "1.10.2-AOE",
            "1.10.2-AOE-1.1.5",
            "1.10.2-All the Mods",
            "1.10.2-FTB_Beyond",
            "1.10.2-LiteLoader1.10.2",
            "1.10.2-forge2511-AOE-1.1.2",
            "1.10.2-forge2511-ATM-E",
            "1.10.2-forge2511-Age_of_Progression",
            "1.10.2-forge2511_Farming_Valley",
            "1.10.2-forge2511_bxztest",
            "1.10.2-forge2511-simple_life_2",
            "1.10.2-forge2511中文",
            "1.12.2",
            "1.12.2_Modern_Skyblock-3.4.2",
            "1.13.1",
            "1.99999999999999999999",
            "2",
            "2.0",
            "2.1",
        ];
        let mut output: Vec<String> = input.iter().map(|s| s.to_string()).collect();
        let expected: Vec<String> = input.iter().map(|s| s.to_string()).collect();
        let comparator = |a: &String, b: &String| VersionNumber::compare(a, b).then_with(|| a.cmp(b));

        // Sorting the reversed list must reproduce the sorted order.
        let mut reversed = output.clone();
        reversed.reverse();
        reversed.sort_by(comparator);
        assert_eq!(reversed, expected);

        // Sorting an already-sorted list must keep it stable.
        output.sort_by(comparator);
        assert_eq!(output, expected);
    }
}
