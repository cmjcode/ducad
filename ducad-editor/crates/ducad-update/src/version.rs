//! Versi rilis bergaya SemVer (`v0.4.6`, `0.5.0-beta.1`).
//!
//! Hanya bagian yang dibutuhkan untuk memutuskan "rilis ini lebih baru":
//! metadata build (`+…`) dibuang, dan pra-rilis dibandingkan sebagai teks
//! (cukup karena updater hanya membaca `releases/latest`, yang memang
//! tidak pernah mengembalikan pra-rilis).

use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Option<String>,
}

impl Version {
    /// Mengurai `v1.2.3`, `1.2`, `1.2.3-rc.1+abc`. `None` bila bukan versi.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let text = text.strip_prefix(['v', 'V']).unwrap_or(text);
        let text = text.split('+').next().unwrap_or(text);
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) if !pre.is_empty() => (core, Some(pre.to_string())),
            Some(_) => return None,
            None => (text, None),
        };
        let mut parts = core.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next().map(str::parse).transpose().ok()?.unwrap_or(0);
        let patch = parts.next().map(str::parse).transpose().ok()?.unwrap_or(0);
        if parts.next().is_some() {
            return None;
        }
        Some(Self { major, minor, patch, pre })
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => Ordering::Equal,
                // Rilis final lebih baru daripada pra-rilisnya sendiri.
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => a.cmp(b),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = &self.pre {
            write!(f, "-{pre}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn parses_tags_and_partial_versions() {
        assert_eq!(v("v0.4.6"), Version { major: 0, minor: 4, patch: 6, pre: None });
        assert_eq!(v("1.2"), Version { major: 1, minor: 2, patch: 0, pre: None });
        assert_eq!(v(" V2.0.1-rc.1+build5 ").pre.as_deref(), Some("rc.1"));
        assert!(Version::parse("latest").is_none());
        assert!(Version::parse("1.2.3.4").is_none());
        assert!(Version::parse("1.2.3-").is_none());
    }

    #[test]
    fn orders_numerically_and_final_after_prerelease() {
        assert!(v("0.4.10") > v("0.4.9"));
        assert!(v("1.0.0") > v("0.99.99"));
        assert!(v("1.0.0") > v("1.0.0-rc.1"));
        assert_eq!(v("v0.4.5").cmp(&v("0.4.5")), Ordering::Equal);
    }

    #[test]
    fn display_round_trips() {
        assert_eq!(v("v0.4.6").to_string(), "0.4.6");
        assert_eq!(v("1.0.0-beta").to_string(), "1.0.0-beta");
    }
}
