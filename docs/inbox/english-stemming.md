# English stemming for search: a survey (2026-09-29)

For the tokenizer plan's D8 (`docs/tokenizer-plan.md`): the user, after
T0, "we've hand-waved what exactly stemming for english is going to look
like - do we know?" The plan's shape so far: an English stemmed field
beside the unstemmed one, the unstemmed weighted higher, applied to texts
D13 calls English. What is not settled: which stemmer, what it leaves
alone, how collisions are counted before a rebuild.

This is a research agent's report from papers, docs and crate sources,
read 2026-09-29. Its measurements on `rust-stemmers` 1.2.0 were run
against Snowball's own test vocabulary, outside the repository; the rest
is the sources' own, and not yet checked here.

## Findings

- **Tantivy's English stemmer is a 2019 snapshot of Porter2.** Tantivy
  0.26.2 uses `rust-stemmers` 1.2.0, released 2019-11-17
  ([crates.io](https://crates.io/crates/rust-stemmers)); its R1 exception
  list has only `arsen`/`commun`/`gener` (read in the cached source).
  Snowball 3.0.0 (2025-05-08) stopped conflating organ/organic/organize,
  universe/universal/university, emerge/emergency, later/lateral and
  past/paste, changed `-ogist` to `-og` and restricted undoubling
  (add/ad); Snowball 3.1.0 (2026-05-22) separated
  intern/internal/international ([Snowball
  NEWS](https://github.com/snowballstem/snowball/blob/master/NEWS)).
  *Measured by the agent:* `rust-stemmers` 1.2.0 over Snowball's English
  test vocabulary ([snowball-data](https://github.com/snowballstem/snowball-data))
  differs from current Snowball on 57 of 42,649 words — organism→`organ`,
  university→`univers`, international→`intern`, emergency→`emerg`.
  Tantivy's `Stemmer` stems every token; its `Token` has no keyword flag,
  so nothing can be protected without a filter of our own.
- **The evidence: a little help on average, a lot of variance by
  query.** In biomedical retrieval, stemming helps verbose queries and
  slightly hurts gene-symbol and keyword queries; light and aggressive
  stemmers usually do not differ significantly.
- **Production systems.** Lucene, Elasticsearch and OpenSearch English
  analyzers default to a possessive filter, a keyword marker and Porter
  (1980). OpenAlex uses KStem, with a `.no_stem` escape. PubMed documents
  no algorithmic stemming: a lookup table of singular/plural pairs and
  US/UK spellings.
- **Rust.** No KStem port exists. The S-stemmer is trivial to write.
  Current Porter2 is available through `frostem`, or generated with
  Snowball's own Rust generator.

## Candidates

ICF (index compression factor) is (words − stems) / words, computed by
the agent on the 42,589 alphabetic words common to Snowball's English,
Porter and Lovins test vocabularies — general English, indicative only.

| Method | What it conflates | Aggressiveness | Real words? | Rust | Notable evidence |
|---|---|---|---|---|---|
| Possessive filter (Lucene `EnglishPossessiveFilter`) | trailing `'s` | minimal | yes | Not needed: the tokenizer already splits at the apostrophe | First filter in Lucene/ES/OpenSearch English analyzers |
| S-stemmer ([Harman 1991](https://doi.org/10.1002/(SICI)1097-4571(199101)42:1%3C7::AID-ASI2%3E3.0.CO;2-P)) = Lucene [`EnglishMinimalStemmer`](https://github.com/apache/lucene/blob/main/lucene/analysis/common/src/java/org/apache/lucene/analysis/en/EnglishMinimalStemmer.java) | plural -s/-es/-ies only; keeps -us/-ss | light (ICF 0.15; 20% of words changed) | mostly (analysis→`analysi`, species→`specy`) | [`polystem`](https://github.com/lgrz/polystem) 0.3.0 (MIT, 2019, dormant), or port Lucene's ~15 lines (Apache-2.0) | ES `minimal_english`. TREC Genomics 2005: best in 4 of 9 models, not significantly different from none. Jiang & Zhai: −2% to 0% on keyword queries, +0.5% to +6.8% on verbose ones |
| KStem ([Krovetz 1993](https://doi.org/10.1145/160688.160718)) | plural, -ed, -ing and selected derivational endings, each candidate checked against a ~27.5k-headword dictionary; tokens with any character outside a–z, or of ≤2 characters, not stemmed ([Lucene source](https://github.com/apache/lucene/blob/main/lucene/analysis/common/src/java/org/apache/lucene/analysis/en/KStemmer.java)) | light–medium | yes | **None on crates.io.** Port from Lucene (Apache-2.0 plus the original UMass BSD-style licence); other ports: [Go, 2026](https://pkg.go.dev/github.com/mightatnight/krovetz), [C++, Apache-2.0](https://github.com/pisa-engine/KrovetzStemmer) | Krovetz: inflectional-only gains of +9.7% (CACM), +33.4% (NPL), +1.8% (TIME), +4.1% (WEST), 58–100% of Porter's gain ([PDF](http://people.scs.carleton.ca/~armyunis/projects/KAPI/Krovetz.pdf)). ES `light_english`; OpenAlex. Best Lucene stemmer at reproducing BNC lemma classes: F 66.5 vs Porter 61.4, Snowball 59.0, none 47.2 ([Endrédy](https://real.mtak.hu/34378/1/ei_corpus_eval_of_stemmers_ltc_u.pdf)) |
| Porter (1980) | ~60 suffixes in 5 steps; inflection and derivation | medium (ICF 0.40) | no | [`porter-stemmer`](https://crates.io/crates/porter-stemmer) (MPL-2.0, 2019); [`nltk-porter`](https://crates.io/crates/nltk-porter) (Apache-2.0, 2026); Snowball `porter` via frostem | Default in Lucene, ES, OpenSearch and [Anserini](https://github.com/castorini/anserini/blob/master/src/main/java/io/anserini/index/IndexCollection.java); [Porter](https://tartarus.org/martin/PorterStemmer/) calls it "frozen" and "slightly inferior" to Porter2 |
| Porter2 / [Snowball English](https://snowballstem.org/algorithms/english/stemmer.html) | Porter plus fixes: -ly, terminal y, keeps -us, invariant words, R1 exception prefixes | medium (ICF 0.41) | no | `rust-stemmers` 1.2.0 (MIT/BSD-3, 2019 algorithms; last commit 2021); [`qdrant-rust-stemmers`](https://github.com/qdrant/rust-stemmers) 1.2.2 (packaging fork, same algorithms); [`frostem`](https://github.com/Xuanwo/frostem) (BSD-3, tracks Snowball main daily, created 2026-08, one maintainer); [`snowball_stemmers_rs`](https://github.com/SeekStorm/snowball-stemmers-rs) (MIT, 2026); [`waken_snowball`](https://crates.io/crates/waken_snowball) (BSD-3, 2025); or generated with Snowball's Rust generator (since 2.0.0, 2019-10-02; latest release 3.1.1, 2026-06-03) | ES `porter2`; T0's miss analysis. No head-to-head IR study against Porter found |
| Lovins (1968) | 294 suffixes, 29 conditions, 35 recoding rules ([Willett 2006](https://eprints.whiterose.ac.uk/id/eprint/1434/)) | aggressive (ICF 0.48) | no | Snowball `lovins` via frostem or snowball_stemmers_rs | [Deprecated in ES 8.16.0](https://www.elastic.co/docs/reference/text-analysis/analysis-stemmer-tokenfilter); worst average MAP in TREC Genomics 2005 (0.179 vs 0.194 for none) |
| Lancaster / Paice-Husk ([1990](https://doi.org/10.1145/101306.101310)) | iterative rule table | most aggressive | no | [`nltk-lancaster`](https://crates.io/crates/nltk-lancaster) (Apache-2.0, 2026, 25 downloads) | Strongest of Lovins/Porter/Paice; most over-stemming, least under-stemming ([Frakes & Fox 2003](https://doi.org/10.1145/945546.945548); [Moral et al. 2014](https://informationr.net/ir/19-1/paper605.html)) |
| Lemmatizers: WordNet morphy; [spaCy](https://spacy.io/api/lemmatizer); [LemmaGen](https://doi.org/10.3217/jucs-016-09-1190); [BioLemmatizer](https://doi.org/10.1186/2041-1480-3-3); NLM [SPECIALIST Lexicon](https://lhncbc.nlm.nih.gov/LSG/Projects/lexicon/current/web/index.html) + [Lexical Tools `norm`](https://lhncbc.nlm.nih.gov/LSG/Projects/lvg/current/docs/userDoc/tools/norm.html) | inflection to a dictionary lemma, irregulars included; POS-dependent | light | yes | [`wordnet-lemmatizer`](https://crates.io/crates/wordnet-lemmatizer) (Apache-2.0, 2026); [`nlprule`](https://github.com/bminixhofer/nlprule) (MIT/Apache code, LGPL-2.1 data, last release 2021); [`english-pos-tagger`](https://crates.io/crates/english-pos-tagger) (MIT, 2026). No Rust for spaCy, LemmaGen or BioLemmatizer. SPECIALIST is open data (>1M items, 2026 release): a lookup table could be built from it | [IIR](https://nlp.stanford.edu/IR-book/html/htmledition/stemming-and-lemmatization-1.html): "at most very modest benefits". [Hollink 2004](https://doi.org/10.1023/B:INRT.0000009439.19151.4c): English 0–5%, lemmatizer "poorer still". BioLemmatizer: 97.5% on CRAFT |
| Corpus-based and selective stemming | refine classes by co-occurrence ([Xu & Croft 1998](https://doi.org/10.1145/267954.267957)); stem the query by context ([Peng 2007](https://doi.org/10.1145/1277741.1277851)); stem selectively ([Göksel 2023](https://pmc.ncbi.nlm.nih.gov/articles/PMC10280253/)) | varies | inherits | none (methods) | Göksel: selective stemming reduces hurt queries |
| LLM and neural | LLM stemming of the vocabulary or in context; [SPLADE](https://arxiv.org/abs/2107.05720) and dense retrievers use subword vocabularies and learned expansion | — | yes (LLM) | none | [Wang, Zhuang, Zuccon 2024](https://arxiv.org/abs/2402.11757): LLM vocabulary stemming below Porter, above Krovetz; its no-stemming baseline looks anomalous |

## The IR evidence

General retrieval: [Harman 1991](https://doi.org/10.1002/(SICI)1097-4571(199101)42:1%3C7::AID-ASI2%3E3.0.CO;2-P)
(Cranfield, Medlars, CACM; S, Lovins, Porter): no significant
improvement, about as many queries helped as hurt ([per Kraaij &
Pohlmann](https://publications.tno.nl/publication/34620754/EuXpbJ/kraaij-1996-viewing.pdf));
down-weighting variants — close to the two-field boost — did not help
either. Krovetz 1993: Porter +1.3% to +45.7% average precision, largest
on short documents. [Hull 1996](https://doi.org/10.1002/(sici)1097-4571(199601)47:1%3C70::aid-asi7%3E3.3.co;2-q)
(TREC): "almost always beneficial, except for long queries at low
recall", about 4–6%, no significant differences between Porter, Lovins
and a linguistic analyzer (read second-hand). [Kraaij & Pohlmann
1996](https://doi.org/10.1145/243199.243209) (Dutch): inflection-only
significantly better than full stemming. Göksel 2023 (NTCIR-13 WWW-1,
KStem vs none): 39% of queries better, 37% worse, 24% tied; nDCG@20
0.3709 vs 0.3755 (p = 0.70).

Biomedical: [Abdou, Ruck & Savoy, TREC Genomics
2005](https://trec.nist.gov/pubs/trec14/papers/uneuchatel.geo.pdf) (Okapi
MAP): none 0.2564, S 0.2572, Porter 0.2551, Lovins 0.2454, differences
usually not significant. [Jiang & Zhai
2007](https://doi.org/10.1007/s10791-007-9027-7) (TREC Genomics 2003–05,
[PDF](https://timan.cs.illinois.edu/czhai/pub/ir-tok.pdf)): on keyword and
gene-symbol queries every stemmer mostly lowered MAP (−4% to +1%, several
significant); on verbose queries Porter −5.8% to +9.0%, S +0.5% to +6.8%,
Lovins −10.8% to +7.9%; they recommend stemming verbose queries only.
[Rivas et al. 2014](https://pmc.ncbi.nlm.nih.gov/articles/PMC3958669/)
(Cystic Fibrosis, BM25): Porter and KStem both 0.1663 MAP vs 0.1545
without. The agent's inference for our sets: TREC PM topics are keywords
(disease, gene, demographics), RELISH queries are whole titles, and the
literature predicts different signs for the two — report by set, never
pooled. Modern setting: [BEIR](https://arxiv.org/abs/2104.08663) finds
BM25 a robust baseline, and Anserini's BM25 baselines use Porter;
learned sparse and dense models make stemming moot, a separate decision.

## Collisions in scientific text (checked on `rust-stemmers` 1.2.0)

Over-stemming in every Porter2 version: experiment/experience→`experi`;
position/positive/positivity→`posit` (`HER2-positive`);
production/product/productivity→`product`;
relative/relation/relativity→`relat`; activity/activation/active→`activ`;
hepatic/hepatitis→`hepat`; physics/physical→`physic`;
medical/medication→`medic`; carbonate/carbonation/carbon→`carbon`;
hydrogenation/hydrogen→`hydrogen`; nitrate/nitration→`nitrat`;
oxide/oxidation→`oxid`. Only in the 2019 snapshot:
organ/organism/organic/organization; university/universe/universal;
intern/internal/international; emerge/emergency; later/lateral;
past/paste; add/ad.

Acronym plurals, under Porter2 and the S-stemmer alike: AIDS→`aid`,
SARS→`sar` (against SAR), ITS→`it`, METs→`met`, NETs→`net`, CARs→`car`,
TAMs→`tam`. The S-stemmer also merges news/new (Porter2 lists "news" as
invariant).

Under-stemming by Porter2: RNAs, SNPs, TLRs left alone by its step-1a
vowel rule while miRNAs→`mirna` is merged; analysis/analyses,
hypothesis/hypotheses, reduce/reduction, produce/production apart;
irregular plurals apart (mice, foci, nuclei, bacteria, matrices); UK/US
spellings apart (-ise/-ize, tumour/tumor) — only lexicon tools
(SPECIALIST, PubMed's table) handle these.

Digits: her2, p53, brca1, il6, covid19 and cd4s unchanged, by luck of the
rules; KStem refuses any token with a digit.

Standard mitigations: protected-word lists (Lucene
`SetKeywordMarkerFilter`, ES
[`keyword_marker`](https://www.elastic.co/docs/reference/text-analysis/analysis-keyword-marker-tokenfilter)
/ `stem_exclusion`); custom mappings
([`stemmer_override`](https://www.elastic.co/docs/reference/text-analysis/analysis-stemmer-override-tokenfilter));
not stemming tokens with digits, short tokens, or acronym-shaped tokens; a
stemmed field beside an exact one (ES's ["mixing exact search with
stemming"](https://www.elastic.co/guide/en/elasticsearch/reference/current/mixing-exact-search-with-stemming.html));
[`keyword_repeat`](https://www.elastic.co/docs/reference/text-analysis/analysis-keyword-repeat-tokenfilter)
puts both forms in one field, but then they cannot be weighted apart.

## The agent's recommendation

Levels for the stemming factor: **none** (the control); **the
S-stemmer** (plural only; cheapest, most precise; catches SNPs and RNAs);
**KStem** (real words, dictionary-guarded, OpenAlex's choice, most of
Porter's gain in Krovetz 1993; needs a port); **Porter2 from current
Snowball (3.1.x)**, pinned to an exact version — through `frostem` or
generated, not `rust-stemmers` 1.2.0. Dropped: Lovins and Lancaster
(aggressive, deprecated in ES, worst in TREC Genomics), Porter 1980
(superseded), LLM stemming (no better than Porter, costly at this scale).
Held for later if the KStem port waits: an inflection table from the
SPECIALIST Lexicon, the only option that handles irregular plurals.

Guards, the same at every level so the factor tests only the algorithm:
no stemming of tokens with a digit; none under 4 characters (the census
to choose 4 or 5); a protected list from the census — terms written with
two or more capitals in most of their occurrences, plus reviewed
collisions — applied alike at index and query time (a custom Tantivy
`TokenFilter` around the stem function, reading the original casing
through the token's byte offsets); the stemmed field weighted below the
unstemmed, the weight screened (0.25, 0.5, 1.0), since Harman's
down-weighting did not help; the stemmer's name and version in the index
manifest, and a refusal when the query side's differs (Snowball changed
57 words' stems between 2019 and 2026).

A collision census before any rebuild, over the unstemmed English term
dictionary with document frequencies and each term's capitalisation:
strength statistics per candidate (Frakes & Fox: classes, mean class
size, ICF, share of terms and of occurrences changed); for each merged
pair, evidence from outside the stemmers — co-document association,
observed against expected (Xu & Croft 1998); agreement with SPECIALIST's
inflection and derivation tables as a gold standard for Paice's UI/OI/ERRT
([NLTK implementation](https://github.com/nltk/nltk/blob/develop/nltk/metrics/paice.py));
shape flags (digits, length, capitals). Output: suspected collisions
ranked by damage (frequent, weakly associated, the lexicon disagreeing)
for review, and the merges each guard blocks, good and bad.

Then, still offline: T0's miss analysis per candidate; each stemmed field
emulated by expanding query terms to their class members over the
existing index (approximate: each variant keeps its own IDF and term
frequency), a rebuild only for the one or two finalists; per-query wins
and losses with bootstrap intervals, by query type.

## Not verified

Hull 1996, Fuller & Zobel 1998 and Paice 1994 read through abstracts and
summaries only. Europe PMC's site was blocked (only its MeSH and UniProt
synonym expansion and its wildcard advice verified); Semantic Scholar
documents an Elasticsearch first stage ([s2search](https://github.com/allenai/s2search))
but not its analyzer; Google Scholar documents nothing ([Uyar
2009](https://doi.org/10.1177/0165551509336801) studied web search). No
stemming ablations found for TREC PM, TREC CDS, CLEF eHealth or BioASQ.
The LLM stemming paper's no-stemming baseline looks anomalous (TREC-COVID
nDCG@10 0.236 against Porter's 0.602): not an effect size. WordNet morphy
described from general knowledge. Which Snowball version
`snowball_stemmers_rs` and `waken_snowball` embed: unchecked
(university→`universiti` means 3.0 or later; internal→`internal` means
3.1). The 2026 lemmatizer and NLTK-port crates have single maintainers,
unassessed.

The agent's scratch data (Snowball's test vocabularies, probe lists,
class and strength scripts) is kept outside the repository for the
collision census.
