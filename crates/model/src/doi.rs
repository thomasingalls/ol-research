//! Digital Object Identifiers.

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt;

/// A DOI, lowercased.
///
/// The rule (plan §4): `10.` then four to nine ASCII digits with no leading
/// zero (the registrant), `/`, then a non-empty suffix of printable ASCII
/// and letters, with no whitespace. DOIs are case-insensitive by
/// definition, so the canonical form is lowercase — for letters beyond
/// ASCII too, as registries write them (`réseaux`, not `Réseaux`), though
/// the DOI Handbook folds only ASCII case (§4.3.4, 2025 edition). The
/// value is borrowed from the input unless something was read or
/// lowercased (about 7 % of PubMed's DOIs).
///
/// Letters beyond ASCII are the DOI of record (model version 10, the
/// user's decision, 2026-09-26): Crossref registers `10.3166/réseaux…` and
/// `10.1556/mentál…`, doi.org resolves them, and some publishers registered
/// an ASCII twin as well (`día` and `dia`) — a second name for the work,
/// which the merge joins, not a spelling this type folds.
///
/// Five things are read rather than rejected, each a misspelling of a DOI
/// the registry knows, and each reported in [`Readings`] so a source can
/// count it:
/// - every Unicode dash, and the minus sign, is `-`. U+2010 since model
///   version 9 (Wiley's 13,305 reference DOIs in Crossref's March 2026
///   sample, whose hyphen forms resolve and U+2010 forms do not); the rest
///   since model 10, when the file's en-dash DOIs proved second
///   registrations of their hyphen twins, which doi.org redirects to
///   (`10.1027/0838–1925.18.2.82` → `…0838-1925…`);
/// - whitespace at either end is dropped (`10.3354/meps7782` and three
///   non-breaking spaces after `10.4021/gr2008.11.1240`, 2026-09-26), where
///   whitespace inside is still refused;
/// - a letter written as its UTF-8 bytes read as Latin-1, `Ã¼` or,
///   lowercased, `ã¼` for `ü` (`10.2195/…_gã¼nthner_…`, whose `günthner` twin
///   is registered and resolves), is repaired: `Ã` or `ã` followed by a
///   character from U+0080 to U+00BF is the Latin-1 letter those two bytes
///   encode, U+00C0 to U+00FF;
/// - a soft hyphen, U+00AD, and a zero-width space, U+200B, are dropped
///   (model version 11, 2026-09-26). Neither is a character of the name:
///   each marks a place a line may break, the soft hyphen shown only where
///   one does and the space never, so `10.11646/zoo\u{ad}taxa.4966.5.3` is
///   Zootaxa's `…/zootaxa.4966.5.3`. Crossref's March 2026 file cites
///   JAMA's DOIs with a soft hyphen after the slash
///   (`10.1001/\u{ad}jamasurg.2022.7640`) and ASM's with a zero-width
///   space in a page range (`10.1128/aem.45.4.1394-\u{200b}1397.1983`).
///
/// A character that is none of these — a symbol (`×` for `x`), a control,
/// a zero-width joiner, whitespace inside — is still refused.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Doi<'a>(#[serde(borrow)] Cow<'a, str>);

/// A registrant code: the digits between `10.` and `/`, `1001` in
/// `10.1001/jama.2025.15703`. Four to nine digits, no leading zero.
///
/// Also the type of Crossref's `prefix`, which names the DOI's *current
/// owner* and differs from the DOI's own registrant on 7 % of works, because
/// DOIs move between publishers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Registrant(u32);

/// What reading a DOI changed from what its source wrote, beyond case
/// ([`Doi::read`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)] // five independent facts
pub struct Readings {
    /// A dash other than `-` was read as `-`.
    pub dash: bool,
    /// Whitespace at an end was dropped.
    pub space: bool,
    /// A broken encoding was repaired.
    pub encoding: bool,
    /// A soft hyphen was dropped.
    pub soft_hyphen: bool,
    /// A zero-width space was dropped.
    pub zero_width_space: bool,
}

impl Readings {
    /// Each reading made, in words, for a report.
    pub fn made(self) -> impl Iterator<Item = &'static str> {
        [
            (self.dash, "a dash read as a hyphen"),
            (self.space, "whitespace at an end dropped"),
            (self.encoding, "a broken encoding repaired"),
            (self.soft_hyphen, "a soft hyphen dropped"),
            (self.zero_width_space, "a zero-width space dropped"),
        ]
        .into_iter()
        .filter_map(|(made, what)| made.then_some(what))
    }

    /// True when nothing was read.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        !(self.dash || self.space || self.encoding || self.soft_hyphen || self.zero_width_space)
    }
}

/// Every Unicode dash (general category Pd) but `-` itself, and the minus
/// sign: each read as `-` in a suffix.
const DASHES: [char; 25] = [
    '\u{058A}', '\u{05BE}', '\u{1400}', '\u{1806}', '\u{2010}', '\u{2011}', '\u{2012}', '\u{2013}',
    '\u{2014}', '\u{2015}', '\u{2E17}', '\u{2E1A}', '\u{2E3A}', '\u{2E3B}', '\u{2E40}', '\u{2E5D}',
    '\u{301C}', '\u{3030}', '\u{30A0}', '\u{FE31}', '\u{FE32}', '\u{FE58}', '\u{FE63}', '\u{FF0D}',
    '\u{2212}',
];

/// Why a string is not a DOI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Error {
    /// Nothing to parse.
    Empty,
    /// Does not start with `10.`.
    BadPrefix,
    /// The registrant code is not four to nine digits without a leading zero.
    Registrant,
    /// No `/` after the registrant code.
    MissingSlash,
    /// Nothing after the `/`.
    EmptySuffix,
    /// A byte that is not printable ASCII, at this offset into the input.
    InvalidChar {
        /// Byte offset into the whole input.
        offset: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("empty"),
            Self::BadPrefix => f.write_str("does not start with 10."),
            Self::Registrant => {
                f.write_str("registrant code is not 4-9 digits without a leading zero")
            }
            Self::MissingSlash => f.write_str("no / after the registrant code"),
            Self::EmptySuffix => f.write_str("empty suffix"),
            Self::InvalidChar { offset } => {
                write!(
                    f,
                    "a character that is not printable ASCII or a letter at {offset}"
                )
            }
        }
    }
}

impl std::error::Error for Error {}

impl Registrant {
    /// Parses a prefix, `10.1001`.
    ///
    /// # Errors
    ///
    /// [`Error::BadPrefix`] or [`Error::Registrant`].
    pub fn parse(s: &str) -> Result<Self, Error> {
        let digits = s.strip_prefix("10.").ok_or(Error::BadPrefix)?;
        Self::from_digits(digits)
    }

    fn from_digits(digits: &str) -> Result<Self, Error> {
        if !(4..=9).contains(&digits.len())
            || !digits.bytes().all(|b| b.is_ascii_digit())
            || digits.starts_with('0')
        {
            return Err(Error::Registrant);
        }
        digits.parse().map(Self).map_err(|_| Error::Registrant)
    }

    /// The code.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// The registrant with code `code`, as [`Registrant::get`] gives it:
    /// four to nine digits, no leading zero.
    #[must_use]
    pub const fn from_code(code: u32) -> Option<Self> {
        if code >= 1_000 && code <= 999_999_999 {
            Some(Self(code))
        } else {
            None
        }
    }
}

impl fmt::Display for Registrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "10.{}", self.0)
    }
}

impl<'a> Doi<'a> {
    /// The prefix OpenAlex and others put in front of a DOI.
    pub const URL_PREFIX: &'static str = "https://doi.org/";

    /// U+2010 HYPHEN, read as `-` in a suffix (see [`Doi`]).
    pub const HYPHEN: char = '\u{2010}';

    /// Parses a bare DOI such as `10.1001/jama.2025.15703`, making the
    /// readings of [`Doi`] without saying which; [`Self::read`] says.
    ///
    /// # Errors
    ///
    /// As [`Self::read`].
    pub fn parse(s: &'a str) -> Result<Self, Error> {
        Self::read(s).map(|(doi, _)| doi)
    }

    /// Parses a bare DOI, and says what it read from what was written: a
    /// dash, whitespace at an end, a broken encoding, a soft hyphen, a
    /// zero-width space ([`Doi`]).
    ///
    /// # Errors
    ///
    /// See [`Error`]. An offset is into the input as given, before
    /// anything is read.
    pub fn read(s: &'a str) -> Result<(Self, Readings), Error> {
        let trimmed = s.trim();
        let lead = s.len() - s.trim_start().len();
        let mut readings = Readings {
            space: trimmed.len() != s.len(),
            ..Readings::default()
        };
        let s = trimmed;
        if s.is_empty() {
            return Err(Error::Empty);
        }
        let rest = s.strip_prefix("10.").ok_or(Error::BadPrefix)?;
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        Registrant::from_digits(&rest[..digits])?;
        let suffix = rest[digits..]
            .strip_prefix('/')
            .ok_or(Error::MissingSlash)?;
        if suffix.is_empty() {
            return Err(Error::EmptySuffix);
        }
        let suffix_start = lead + s.len() - suffix.len();
        // A suffix that is all ASCII — nearly every one — is checked a byte
        // at a time, and borrowed when it needs no lowercasing.
        if suffix.is_ascii() {
            if let Some(bad) = suffix.bytes().position(|b| !(0x21..=0x7e).contains(&b)) {
                return Err(Error::InvalidChar {
                    offset: suffix_start + bad,
                });
            }
            let doi = if suffix.bytes().any(|b| b.is_ascii_uppercase()) {
                Cow::Owned(s.to_ascii_lowercase())
            } else {
                Cow::Borrowed(s)
            };
            return Ok((Self(doi), readings));
        }
        let mut out = String::with_capacity(s.len());
        out.push_str(&s[..s.len() - suffix.len()]);
        let mut chars = suffix.char_indices().peekable();
        while let Some((at, c)) = chars.next() {
            if let Some(&(_, next)) = chars.peek()
                && (c == '\u{C3}' || c == '\u{E3}')
                && ('\u{80}'..='\u{BF}').contains(&next)
            {
                let letter = char::from_u32(0xC0 + u32::from(next) - 0x80).unwrap_or(next);
                out.extend(letter.to_lowercase());
                readings.encoding = true;
                chars.next();
            } else if ('\x21'..='\x7e').contains(&c) {
                out.push(c.to_ascii_lowercase());
            } else if DASHES.contains(&c) {
                out.push('-');
                readings.dash = true;
            } else if c == '\u{AD}' {
                readings.soft_hyphen = true;
            } else if c == '\u{200B}' {
                readings.zero_width_space = true;
            } else if c.is_alphabetic() {
                out.extend(c.to_lowercase());
            } else {
                return Err(Error::InvalidChar {
                    offset: suffix_start + at,
                });
            }
        }
        if out.len() == s.len() - suffix.len() {
            return Err(Error::EmptySuffix);
        }
        Ok((Self(Cow::Owned(out)), readings))
    }

    /// Parses the URL form, `https://doi.org/10.…`.
    ///
    /// # Errors
    ///
    /// [`Error::BadPrefix`] unless the input starts with exactly
    /// [`Self::URL_PREFIX`]; then as [`Self::parse`], with offsets relative
    /// to the whole input.
    pub fn from_url(s: &'a str) -> Result<Self, Error> {
        let rest = s.strip_prefix(Self::URL_PREFIX).ok_or(Error::BadPrefix)?;
        Self::parse(rest).map_err(|e| match e {
            Error::InvalidChar { offset } => Error::InvalidChar {
                offset: offset + Self::URL_PREFIX.len(),
            },
            other => other,
        })
    }

    /// The canonical, lowercased DOI.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The registrant code.
    #[must_use]
    pub fn registrant(&self) -> Registrant {
        let s = self.as_str();
        Registrant::from_digits(&s[3..s.find('/').unwrap_or(s.len())]).unwrap_or(Registrant(0))
    }

    /// The DOI spelled in ASCII, where it has letters beyond ASCII and
    /// every one of them has an ASCII spelling (`é` `e`, `ß` `ss`; the
    /// table in `doi_ascii`): `10.3166/réseaux.151.37-62` is
    /// `10.3166/reseaux.151.37-62`. `None` for an ASCII DOI, and for one
    /// with a letter the table does not spell, a Greek or a Han one.
    ///
    /// Publishers registered some such spellings as DOIs of their own: of
    /// the 99 DOIs of Crossref's March 2026 file with a letter beyond
    /// ASCII, 16 have an ASCII twin with the same title, and one more with
    /// the same article's title written otherwise (`babel.54.1.04día`,
    /// homelab `ol-rht-census-doi-spellings-v1`, 2026-09-26). The merge
    /// gives a work its DOI's ASCII spelling as a name of its own, so the
    /// twins are one work and a citation of either lands on it (the
    /// user's decision; model version 10).
    #[must_use]
    pub fn ascii_spelling(&self) -> Option<Doi<'static>> {
        let s = self.as_str();
        if s.is_ascii() {
            return None;
        }
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            if c.is_ascii() {
                out.push(c);
            } else {
                let i = crate::doi_ascii::TABLE
                    .binary_search_by(|(k, _)| k.cmp(&c))
                    .ok()?;
                out.push_str(crate::doi_ascii::TABLE[i].1);
            }
        }
        Some(Doi(Cow::Owned(out)))
    }

    /// True when no copy was needed: an ASCII suffix, nothing to lowercase
    /// or read.
    #[must_use]
    pub const fn is_borrowed(&self) -> bool {
        matches!(self.0, Cow::Borrowed(_))
    }
}

impl fmt::Display for Doi<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::{Doi, Error, Readings, Registrant};

    #[test]
    fn parses_observed_forms_borrowing_lowercase_ones() {
        for s in [
            "10.1001/jama.2025.15703",
            "10.1093/bioinformatics/btu170",
            "10.1038/s41586-021-03819-2",
            "10.82227/repository.uwtsd.ac.uk.00001267",
            "10.1016/s0140-6736(25)01234-5",
        ] {
            let doi = Doi::parse(s).unwrap();
            assert_eq!(doi.as_str(), s);
            assert!(doi.is_borrowed(), "{s}");
        }
        assert_eq!(
            Doi::parse("10.1001/jama.2025.15703")
                .unwrap()
                .registrant()
                .get(),
            1001
        );
        assert_eq!(Registrant::parse("10.1037").unwrap().to_string(), "10.1037");
        assert_eq!(Registrant::parse("10.0123"), Err(Error::Registrant));
        assert_eq!(Registrant::parse("1037"), Err(Error::BadPrefix));
    }

    #[test]
    fn uppercase_is_lowercased_by_copy() {
        let doi = Doi::parse("10.1097/MEG.0000000000003129").unwrap();
        assert_eq!(doi.as_str(), "10.1097/meg.0000000000003129");
        assert!(!doi.is_borrowed());
        assert_eq!(doi, Doi::parse("10.1097/meg.0000000000003129").unwrap());
    }

    #[test]
    fn rejects_what_sources_put_in_doi_slots() {
        assert_eq!(Doi::parse("DOI"), Err(Error::BadPrefix));
        assert_eq!(Doi::parse("e25.00642"), Err(Error::BadPrefix));
        assert_eq!(
            Doi::parse("10.10.1080/09638288.2024.2381616"),
            Err(Error::Registrant)
        );
        assert_eq!(
            Doi::parse("%9210.1046/j.1532-5415.2003.51185.x"),
            Err(Error::BadPrefix)
        );
        assert_eq!(
            Doi::parse("https://www.ajronline.org/doi/10.2214/AJR.07.2123"),
            Err(Error::BadPrefix)
        );
        // A look-alike that is no dash, and a zero-width character, as
        // Crossref's March 2026 sample carries them in reference DOIs:
        // refused, not read.
        assert_eq!(
            Doi::parse("10.1177/0308518\u{d7}18809094"),
            Err(Error::InvalidChar { offset: 15 })
        );
        assert_eq!(
            Doi::parse("10.1016/j.obmed.2025.100668\u{200c}"),
            Err(Error::InvalidChar { offset: 27 })
        );
    }

    #[test]
    fn a_unicode_hyphen_is_read_as_a_hyphen() {
        // Wiley's reference DOIs, from Crossref's March 2026 sample.
        let doi = Doi::parse("10.1038/s41467\u{2010}020\u{2010}14809\u{2010}9").unwrap();
        assert_eq!(doi.as_str(), "10.1038/s41467-020-14809-9");
        assert!(!doi.is_borrowed());
        assert_eq!(doi, Doi::parse("10.1038/s41467-020-14809-9").unwrap());
        // Read and lowercased together.
        assert_eq!(
            Doi::parse("10.1111/1755\u{2010}0998.13324X")
                .unwrap()
                .as_str(),
            "10.1111/1755-0998.13324x"
        );
        assert_eq!(
            Doi::from_url("https://doi.org/10.1002/jha2\u{2010}1033")
                .unwrap()
                .as_str(),
            "10.1002/jha2-1033"
        );
        // Beside a character that is not read, the offset is into the input
        // as given: U+2010 is three bytes here, not one.
        assert_eq!(
            Doi::parse("10.1136/bmjopen\u{2010}2018\u{d7}025300"),
            Err(Error::InvalidChar { offset: 22 })
        );
        // Only in the suffix: the registrant is digits.
        assert_eq!(Doi::parse("10.1111\u{2010}/x"), Err(Error::MissingSlash));
    }

    #[test]
    fn rule_edges() {
        assert_eq!(Doi::parse(""), Err(Error::Empty));
        assert_eq!(Doi::parse("10.123/x"), Err(Error::Registrant));
        assert_eq!(Doi::parse("10.0123/x"), Err(Error::Registrant));
        assert_eq!(Doi::parse("10.1234567890/x"), Err(Error::Registrant));
        assert_eq!(Doi::parse("10.1234"), Err(Error::MissingSlash));
        assert_eq!(Doi::parse("10.1234x/y"), Err(Error::MissingSlash));
        assert_eq!(Doi::parse("10.1234/"), Err(Error::EmptySuffix));
        assert_eq!(
            Doi::parse("10.1234/a b"),
            Err(Error::InvalidChar { offset: 9 })
        );
        assert_eq!(
            Doi::parse("10.1234/a\u{bc}"),
            Err(Error::InvalidChar { offset: 9 })
        );
        assert_eq!(Doi::parse(" "), Err(Error::Empty));
        assert_eq!(
            Doi::parse(" 10.1234/a b"),
            Err(Error::InvalidChar { offset: 10 }),
            "offsets are into the input as given"
        );
        assert!(
            Doi::parse("10.1234/a.").is_ok(),
            "trailing period is printable ASCII"
        );
    }

    /// The record DOIs of Crossref's March 2026 file that are not ASCII
    /// (homelab `ol-rht-dump-crossref-v3`, 2026-09-26).
    #[test]
    fn what_the_march_2026_file_writes_beyond_ascii() {
        let read = |s| Doi::read(s).unwrap();
        // Every dash is `-`: the en dash on Hogrefe's second registration,
        // which doi.org redirects to its hyphen twin; the minus sign and
        // the em dash as well.
        let (doi, r) = read("10.1027/0838\u{2013}1925.18.2.82");
        assert_eq!(doi.as_str(), "10.1027/0838-1925.18.2.82");
        assert_eq!(
            r,
            Readings {
                dash: true,
                ..Readings::default()
            }
        );
        for dash in ['\u{2212}', '\u{2014}', '\u{2011}', '\u{FF0D}'] {
            assert_eq!(
                Doi::parse(&format!("10.1234/a{dash}b")).unwrap().as_str(),
                "10.1234/a-b",
                "{dash:?}"
            );
        }
        // A letter is the DOI of record, lowercased as registries write it.
        let (doi, r) = read("10.3166/r\u{e9}seaux.151.139-166");
        assert_eq!(doi.as_str(), "10.3166/r\u{e9}seaux.151.139-166");
        assert!(r.is_empty());
        assert_eq!(
            Doi::parse("10.1556/MENT\u{c1}L.5.2004.1.1")
                .unwrap()
                .as_str(),
            "10.1556/ment\u{e1}l.5.2004.1.1"
        );
        assert_eq!(
            Doi::parse("10.1234/stra\u{df}e").unwrap().as_str(),
            "10.1234/stra\u{df}e",
            "an eszett is already lowercase"
        );
        // Non-breaking spaces at the end, dropped.
        let (doi, r) = read("10.4021/gr2008.11.1240\u{a0}\u{a0}\u{a0}");
        assert_eq!(doi.as_str(), "10.4021/gr2008.11.1240");
        assert_eq!(
            r,
            Readings {
                space: true,
                ..Readings::default()
            }
        );
        // `ü` written as its UTF-8 bytes read as Latin-1 and lowercased.
        let (doi, r) = read("10.2195/lj_not_ref_d_g\u{e3}\u{bc}nthner_0120052");
        assert_eq!(doi.as_str(), "10.2195/lj_not_ref_d_g\u{fc}nthner_0120052");
        assert_eq!(
            r,
            Readings {
                encoding: true,
                ..Readings::default()
            }
        );
        assert_eq!(
            Doi::parse("10.1234/caf\u{c3}\u{a9}").unwrap().as_str(),
            "10.1234/caf\u{e9}",
            "`Ã©` for `é`, not lowercased"
        );
        assert_eq!(
            Doi::parse("10.1234/s\u{e3}o").unwrap().as_str(),
            "10.1234/s\u{e3}o",
            "`ã` before a letter is itself"
        );
        assert_eq!(
            read("\u{a0}10.1234/a\u{2013}b")
                .1
                .made()
                .collect::<Vec<_>>(),
            ["a dash read as a hyphen", "whitespace at an end dropped"]
        );
    }

    /// Reference DOIs of Crossref's March 2026 file with a soft hyphen or a
    /// zero-width space, as the normalize reports printed them (homelab
    /// `ol-rht-normalize-crossref-v1`, 2026-09-26).
    #[test]
    fn a_soft_hyphen_or_a_zero_width_space_is_dropped() {
        for (written, read) in [
            (
                "10.11646/zoo\u{ad}taxa.4966.5.3",
                "10.11646/zootaxa.4966.5.3",
            ),
            (
                "10.1001/\u{ad}jamasurg.2022.7640",
                "10.1001/jamasurg.2022.7640",
            ),
            (
                "10.1001/\u{ad}jamanetworkopen.2019.8340",
                "10.1001/jamanetworkopen.2019.8340",
            ),
            ("10.4137/\u{ad}jmecd.S18077", "10.4137/jmecd.s18077"),
        ] {
            let (doi, r) = Doi::read(written).unwrap();
            assert_eq!(doi.as_str(), read);
            assert_eq!(
                r,
                Readings {
                    soft_hyphen: true,
                    ..Readings::default()
                }
            );
            assert_eq!(r.made().collect::<Vec<_>>(), ["a soft hyphen dropped"]);
        }
        let (doi, r) = Doi::read("10.1128/aem.45.4.1394-\u{200b}1397.1983").unwrap();
        assert_eq!(doi.as_str(), "10.1128/aem.45.4.1394-1397.1983");
        assert_eq!(
            r,
            Readings {
                zero_width_space: true,
                ..Readings::default()
            }
        );
        assert_eq!(r.made().collect::<Vec<_>>(), ["a zero-width space dropped"]);
        assert_eq!(
            Doi::read("10.1234/\u{ad}"),
            Err(Error::EmptySuffix),
            "nothing but a soft hyphen is no suffix"
        );
        assert_eq!(Doi::read("10.1234/\u{200b}\u{ad}"), Err(Error::EmptySuffix));
        assert!(
            Doi::read("10.1234/a\u{200d}b").is_err(),
            "a zero-width joiner is still refused"
        );
        assert_eq!(
            Doi::read("10.1234/a\u{ad}\u{2013}b").unwrap().1,
            Readings {
                dash: true,
                soft_hyphen: true,
                ..Readings::default()
            }
        );
    }

    #[test]
    fn a_letter_beyond_ascii_has_an_ascii_spelling() {
        let spelled = |s| {
            Doi::parse(s)
                .unwrap()
                .ascii_spelling()
                .map(|d| d.as_str().to_owned())
        };
        assert_eq!(
            spelled("10.3166/r\u{e9}seaux.151.37-62").as_deref(),
            Some("10.3166/reseaux.151.37-62")
        );
        assert_eq!(
            spelled("10.1075/babel.54.1.04d\u{ed}a").as_deref(),
            Some("10.1075/babel.54.1.04dia")
        );
        assert_eq!(
            spelled("10.1234/stra\u{df}e").as_deref(),
            Some("10.1234/strasse")
        );
        assert_eq!(spelled("10.1234/plain"), None, "already ASCII");
        assert_eq!(spelled("10.1234/\u{3b1}"), None, "a Greek letter has none");
    }

    #[test]
    fn the_ascii_table_is_sorted_lowercase_letters_to_ascii_letters() {
        let table = &crate::doi_ascii::TABLE;
        for w in table.windows(2) {
            assert!(w[0].0 < w[1].0, "{:?} before {:?}", w[0].0, w[1].0);
        }
        for (c, spelled) in table {
            assert!(c.is_alphabetic() && !c.is_ascii(), "{c:?}");
            assert_eq!(c.to_lowercase().collect::<String>(), c.to_string(), "{c:?}");
            assert!(
                !spelled.is_empty() && spelled.bytes().all(|b| b.is_ascii_lowercase()),
                "{c:?} {spelled:?}"
            );
        }
    }

    #[test]
    fn url_form() {
        let doi = Doi::from_url("https://doi.org/10.25675/3.026947").unwrap();
        assert_eq!(doi.as_str(), "10.25675/3.026947");
        assert_eq!(
            Doi::from_url("http://doi.org/10.1/x"),
            Err(Error::BadPrefix)
        );
        assert_eq!(Doi::from_url("10.25675/3.026947"), Err(Error::BadPrefix));
        assert_eq!(
            Doi::from_url("https://doi.org/10.1234/a b"),
            Err(Error::InvalidChar { offset: 16 + 9 })
        );
    }
}
