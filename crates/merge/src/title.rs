//! The corroboration signal of merge plan §3.3: title agreement,
//! deliberately weak, because it only ever decides among two or three
//! records that already share a name.

use serde::{Deserialize, Serialize};

/// A record's titles, borrowed: the title, and the vernacular title a
/// source keeps beside a translation (PubMed's `VernacularTitle`,
/// Crossref's first alternate title).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Titles<'a> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub title: Option<&'a str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub vernacular: Option<&'a str>,
}

/// A title reduced to what two sources agree on: markup stripped,
/// entities decoded, a whole-title bracket (PubMed's translation)
/// unwrapped, case folded, everything but letters and digits removed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fingerprint(Box<str>);

impl Fingerprint {
    /// The fingerprint of `raw`; `None` when nothing is left.
    #[must_use]
    pub fn of(raw: &str) -> Option<Self> {
        let text = decode(raw);
        let text = text.trim().trim_end_matches('.').trim();
        let text = match text.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
            Some(inner) if !inner.contains('[') => inner,
            Some(_) | None => text,
        };
        let folded: String = text
            .chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect();
        (!folded.is_empty()).then(|| Self(folded.into_boxed_str()))
    }

    /// The text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Markup removed and entities decoded: `<sub>4</sub>` keeps its `4`,
/// `&amp;` becomes `&`, `&#x3b1;` becomes `α`, an unknown entity is
/// dropped whole.
fn decode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(i) = rest.find(['<', '&']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if tail.starts_with('<') {
            match tail.find('>') {
                Some(end) => rest = &tail[end + 1..],
                None => return out,
            }
        } else if let Some(end) = tail[1..].find(';').filter(|&end| end <= 10) {
            let entity = &tail[1..=end];
            if let Some(c) = entity_char(entity) {
                out.push(c);
            }
            rest = &tail[end + 2..];
        } else {
            out.push('&');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

fn entity_char(entity: &str) -> Option<char> {
    Some(match entity {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        _ => {
            let digits = entity.strip_prefix('#')?;
            let code = match digits.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => digits.parse().ok()?,
            };
            char::from_u32(code)?
        }
    })
}

/// A record's titles, fingerprinted and owned: what the identity pass
/// keeps for the records of conflict components.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnedTitles {
    #[allow(missing_docs)]
    pub title: Option<Fingerprint>,
    #[allow(missing_docs)]
    pub vernacular: Option<Fingerprint>,
}

impl OwnedTitles {
    #[allow(missing_docs)]
    #[must_use]
    pub fn of(titles: Titles<'_>) -> Self {
        Self {
            title: titles.title.and_then(Fingerprint::of),
            vernacular: titles.vernacular.and_then(Fingerprint::of),
        }
    }
}

/// How two records' titles compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Agreement {
    /// The same after fingerprinting.
    Equal,
    /// One is a prefix of the other: a subtitle one source dropped.
    Prefix,
    /// One's title is the other's vernacular title: a translation on one
    /// side.
    Vernacular,
    /// A few characters apart on a long title: a plain `S` for a fraktur
    /// one, `Toward` for `Towards`, a typo.
    Similar,
    /// None of the above, and both have a title.
    Disagree,
    /// At least one has no title to test.
    Untestable,
}

impl Agreement {
    /// Whether the rules may treat the records as one work.
    #[must_use]
    pub const fn is_agreement(self) -> bool {
        matches!(
            self,
            Self::Equal | Self::Prefix | Self::Vernacular | Self::Similar
        )
    }
}

/// The shortest fingerprint the prefix rule accepts, so a two-word title
/// is not a prefix of everything that starts with those words.
const PREFIX_MIN: usize = 12;
/// The shortest fingerprint the similarity rule accepts, and how far
/// apart two may be: one edit in ten characters.
const SIMILAR_MIN: usize = 20;
const SIMILAR_PER: usize = 10;

/// Edits between two texts, by character.
fn levenshtein(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut row = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            row[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(row[j] + 1);
        }
        std::mem::swap(&mut prev, &mut row);
    }
    prev[b.len()]
}

/// Compares two records' titles.
#[must_use]
pub fn agree(a: &OwnedTitles, b: &OwnedTitles) -> Agreement {
    let (Some(x), Some(y)) = (&a.title, &b.title) else {
        return Agreement::Untestable;
    };
    if x == y {
        return Agreement::Equal;
    }
    let (short, long) = if x.as_str().len() <= y.as_str().len() {
        (x, y)
    } else {
        (y, x)
    };
    if short.as_str().len() >= PREFIX_MIN && long.as_str().starts_with(short.as_str()) {
        return Agreement::Prefix;
    }
    if a.vernacular.as_ref() == Some(y) || b.vernacular.as_ref() == Some(x) {
        return Agreement::Vernacular;
    }
    let longer = long.as_str().chars().count();
    if longer >= SIMILAR_MIN && levenshtein(x.as_str(), y.as_str()) * SIMILAR_PER <= longer {
        return Agreement::Similar;
    }
    Agreement::Disagree
}

#[cfg(test)]
mod tests {
    use super::{Agreement, Fingerprint, OwnedTitles, Titles, agree};

    fn t(title: &str) -> OwnedTitles {
        OwnedTitles::of(Titles {
            title: Some(title),
            vernacular: None,
        })
    }

    #[test]
    fn fingerprints_strip_what_sources_disagree_on() {
        let f = |s: &str| Fingerprint::of(s).map(|f| f.as_str().to_owned());
        assert_eq!(
            f("A Purely σ-Aromatic, Planar {Ge<sub>4</sub>}<sup>2+</sup>-Ring."),
            f("A Purely σ-Aromatic, Planar {Ge\n <sub>4</sub>\n }\n <sup>2+</sup>\n -Ring")
        );
        assert_eq!(
            f("Professional judgement in Forensic &amp; Legal Medicine (FLM)"),
            f("Professional judgement in Forensic & Legal Medicine (FLM)")
        );
        assert_eq!(
            f("Neuropathology in an &#x3b1;-synuclein model"),
            f("Neuropathology in an α-synuclein model")
        );
        assert_eq!(
            f("[Complete recovery of hungry bone syndrome]."),
            f("Complete recovery of hungry bone syndrome")
        );
        assert_eq!(
            f("Erratum: \u{201c}Relativistic\u{201d} effects"),
            f("Erratum: \"Relativistic\" effects")
        );
        assert_eq!(
            f("Static proximal fixation of inter-trochanteric"),
            f("Static Proximal Fixation of Intertrochanteric")
        );
        assert_eq!(
            f("<i>O</i>, <i>N</i> binding"),
            Some("onbinding".to_owned())
        );
        assert_eq!(
            f("&unknown; x &#zz; y & z"),
            Some("xyz".to_owned()),
            "unknown entities drop, a bare ampersand stays"
        );
        assert_eq!(f(" ... "), None);
        assert_eq!(f("<b></b>"), None);
        assert_eq!(
            f("[a] and [b]"),
            Some("aandb".to_owned()),
            "not a whole-title bracket"
        );
    }

    #[test]
    fn agreement_has_the_three_ways_to_agree() {
        assert_eq!(
            agree(&t("A power primer."), &t("A power primer")),
            Agreement::Equal
        );
        assert_eq!(
            agree(
                &t("Food Insecurity Is a Medical Problem: Lessons from a Clinic"),
                &t("Food Insecurity Is a Medical Problem")
            ),
            Agreement::Prefix
        );
        assert_eq!(
            agree(&t("Aspirin"), &t("Aspirin in patients")),
            Agreement::Disagree,
            "too short for the prefix rule"
        );
        let pubmed = OwnedTitles::of(Titles {
            title: Some("[Recommendations for management of acute pharyngitis]."),
            vernacular: Some("Recomendaciones para el manejo de la faringoamigdalitis aguda"),
        });
        assert_eq!(
            agree(
                &pubmed,
                &t("Recomendaciones para el manejo de la faringoamigdalitis aguda")
            ),
            Agreement::Vernacular
        );
        assert_eq!(
            agree(
                &t("Recomendaciones para el manejo de la faringoamigdalitis aguda"),
                &pubmed
            ),
            Agreement::Vernacular
        );
        assert_eq!(
            agree(
                &t("GWAS and genomic prediction"),
                &t("ICU-Related Neuromuscular Weakness")
            ),
            Agreement::Disagree
        );
        assert_eq!(
            agree(
                &t(
                    "Iterative construction of 𝕾<sub>p</sub>× 𝕾<sub>p</sub>group-adapted irreducible matrix units for the walled Brauer algebra."
                ),
                &t(
                    "Iterative construction of S p × S p group-adapted irreducible matrix units for the walled Brauer algebra"
                )
            ),
            Agreement::Similar
        );
        assert_eq!(
            agree(
                &t(
                    "ATRNet-STAR: A Large Dataset and Benchmark Towards Remote Sensing Object Recognition in the Wild."
                ),
                &t(
                    "ATRNet-STAR: A Large Dataset and Benchmark Toward Remote Sensing Object Recognition in the Wild"
                )
            ),
            Agreement::Similar
        );
        assert_eq!(
            agree(&t("Study of X in mice"), &t("Study of Y in mice")),
            Agreement::Disagree,
            "too short for the similarity rule"
        );
        assert_eq!(
            agree(
                &t("Recommendations for management of acute pharyngitis in adults"),
                &t("Recomendaciones para el manejo de la faringoamigdalitis aguda del adulto")
            ),
            Agreement::Disagree
        );
        assert_eq!(super::levenshtein("kitten", "sitting"), 3);
        assert_eq!(super::levenshtein("", "abc"), 3);
        assert_eq!(
            agree(&OwnedTitles::default(), &t("x")),
            Agreement::Untestable
        );
        assert!(Agreement::Prefix.is_agreement() && !Agreement::Untestable.is_agreement());
    }
}
