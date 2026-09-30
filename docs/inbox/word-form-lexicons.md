# Word-form lexicons: a survey (2026-09-29)

For the tokenizer plan (`docs/tokenizer-plan.md`, D8 and D10): the user,
after the stemming survey, "what about irregular plurals (mice vs mouse,
etc) or british vs american spellings (labour labor) ... do we want to
keep lemmatization out of this for now? nlm's specialist lexicon sounds
interesting, i wonder if there's more than one of those". No stemmer maps
`mice` to `mouse` or `labour` to `labor`; a lookup table does — a word
form to one canonical form, applied alike to text and query, with no
part-of-speech tagging. This surveys the lexicons such a table could be
built from, and which could also serve as a gold standard for stemmer
collisions.

This is a research agent's report, read 2026-09-29. It downloaded and
checked the small files itself (LRSPL, LRNOM, LRWD, varcon.txt,
scowl-pre.txt, AGID's infl.txt, UniMorph eng, the Open English WordNet
exception lists; kept outside the repository) and read samples of the
large SPECIALIST files by byte range. The rest is the sources' own, and
not yet checked here.

## In short

NLM's SPECIALIST Lexicon has no real equal: the only open resource found
with broad biomedical coverage that holds both irregular and Greco-Latin
inflections (its LRAGR table) and spelling variants (LRSPL). The best
partner for general English is VarCon, now part of SCOWL as the English
Speller Database (ESDB). The rest are weaker for this purpose,
share-alike, or paid.

## Comparison

| Lexicon | Covers | Languages | Size | Format | Licence | Latest / maintained |
|---|---|---|---|---|---|---|
| **SPECIALIST Lexicon** | irregular and Greco-Latin inflections; spelling variants incl. hyphenation; abbreviations and acronyms; nominalizations; derivations; synonyms | English, general and biomedical | 534,345 records, 1,007,889 forms (2026) | pipe-delimited tables, unit records, XML, ASCII | US government terms: free, redistributable with conditions, no commercial restriction written | 2026 release, files dated 2025-12-17; yearly since 1994 |
| **SCOWL / ESDB v2** | inflections (from AGID) and GB/US/CA/AU variants (from VarCon) in one file | English | scowl-pre.txt 450,793 lines, 14 MB | custom text, and SQLite | MIT-like; extra notices for AU, UKACD and WordNet-derived parts | 2026-02-25 dictionary-only release; active |
| **VarCon** | variants in five spelling tags (American; British -ise; British -ize/Oxford; Canadian; Australian), with sense notes | English | 18,140 lines, 35,586 forms, 846 KB | `A C: anemia / B D: anaemia` | permissive notice | 2020.12.07; now inside ESDB |
| **AGID** | inflections incl. irregular, with confidence markers | English | 112,505 lines, 3.4 MB | `mouse N: mice` | permissive notice | 2016.01.19 |
| **WordNet exception lists** (Open English WordNet 2025) | irregular inflections only | English | 1,391 noun, 1,884 verb, 1,191 adj, 7 adv lines | `mice mouse` | OEWN CC BY 4.0; Princeton permissive | OEWN 2025, published 2025-12-31 |
| **Wiktextract / kaikki.org** | inflections, "alternative/British spelling of" links, much more | thousands | English 1,390,507 words; all languages 2.8 GB compressed | JSONL | CC BY-SA 4.0 + GFDL (share-alike) | extracted 2026-09-28 from the 2026-09-02 dump; ongoing |
| **UniMorph** | inflection tables | 169 on the site, 183 in the 4.0 paper | eng 652,477 rows, 399,561 lemmas | TSV | per language, mostly CC BY-SA 3.0 | 4.0 (2022); eng last pushed 2023-02 |
| **MorphyNet** | inflection and derivation | 15 | 13.5 M inflections; English 649,594 | TSV | CC BY-SA 3.0 | last pushed 2023-04 |
| **CELEX2** | lemmas, word forms, morphology | English, German, Dutch | German 51,728 lemmas | ASCII | LDC licence, paid | 1996 |
| **BioLexicon** | biomedical terms and variants; British/US and hyphen variants curated for verbs | English | 2.2 M entries | MySQL dump (LMF) | ELRA: €200–320 research, €4,500–11,000 commercial | v1.0, 2009; dormant |
| **UMLS Metathesaurus** | concepts and synonyms; normalized indexes built with LVG | many | — | RRF | UMLS licence required | twice a year |
| **MeSH** | entry terms (synonyms, not variants) | English | — | XML | free; attribution and currency required | yearly |

Sources: SPECIALIST [home](https://lhncbc.nlm.nih.gov/LSG/Projects/lexicon/current/web/index.html),
[growth statistics](https://lhncbc.nlm.nih.gov/LSG/Projects/lexicon/current/docs/designDoc/UDF/statistics/growth.html),
[2026 release files](https://lhncbc.nlm.nih.gov/LSG/Projects/lexicon/current/web/release/2026.html);
ESDB [README and Copyright](https://github.com/en-wl/wordlist),
[2026.02.25 release](https://github.com/en-wl/wordlist/releases/tag/rel-2026.02.25);
[VarCon README](http://wordlist.aspell.net/varcon-readme/);
[AGID README](http://wordlist.aspell.net/agid-readme/);
[OEWN releases](https://github.com/globalwordnet/english-wordnet/releases);
kaikki.org [raw data](https://kaikki.org/dictionary/rawdata.html) and
[English](https://kaikki.org/dictionary/English/index.html);
[Wiktionary copyrights](https://en.wiktionary.org/wiki/Wiktionary:Copyrights);
[UniMorph](https://unimorph.github.io/), [UniMorph 4.0
paper](https://aclanthology.org/2022.lrec-1.89/),
[unimorph/eng](https://github.com/unimorph/eng);
[MorphyNet](https://github.com/kbatsuren/MorphyNet);
[CELEX2](https://catalog.ldc.upenn.edu/LDC96L14); BioLexicon at
[ELRA](https://catalogue.elra.info/en-us/repository/browse/ELRA-T0373/) and
its [paper](https://pmc.ncbi.nlm.nih.gov/articles/PMC3228855/); [UMLS
licence page](https://www.nlm.nih.gov/research/umls/new_users/online_learning/OVR_005.html);
[MeSH terms](https://www.nlm.nih.gov/databases/download/terms_and_conditions_mesh.html)
and [entry terms](https://www.nlm.nih.gov/mesh/intro_entry.html).

The agent's checks:

- **UniMorph eng is poor for this job**: no rows for *children*, *nuclei*
  or *foci*, and junk such as `focus  countable  N;PL` and `matrice →
  matrices`.
- **AGID needs filtering**: rows like `child V?: childed`; its markers
  (`?`, `!`, `~`, numbers) must be honoured.
- **Wiktextract's "British spelling" links are only partly structured**:
  *anaemia* carries `alt_of: anemia`, but *tumour* and *colour* only the
  gloss "Commonwealth standard spelling of …".
- **WordNet lists only true exceptions**: *bacteria* is missing, a lemma
  of its own there.
- **MeSH entry terms are not variants**: NLM calls them "synonyms,
  near-synonyms alternate forms, and other closely related terms" (e.g.
  *Asystole* for *Heart Arrest*) — synonym expansion (D12), not a
  normalization table.

## SPECIALIST in detail

- **What it is**: "a general English lexicon that includes many
  biomedical terms" ([fact sheet,
  2003](https://lhncbc.nlm.nih.gov/LSG/Projects/lexicon/current/docs/SpecialistLexiconFactSheet.pdf)).
  Each record has a base form, a part of speech, a unique id (EUI) and
  optional spelling variants; verbs are "regular, Greco-Latin regular or
  irregular"; nouns carry their plural patterns. Sources include MEDLINE,
  Dorland's, the American Heritage frequency list, Longman's defining
  vocabulary and WordNet.
- **Size (2026)**: 534,345 records, 1,007,889 forms, 48.45% single words;
  over 158K derivation pairs (DM.DB), 283K synonym pairs (SM.DB), 17.7K
  antonym pairs ([word
  stats](https://lhncbc.nlm.nih.gov/LSG/Projects/lexicon/current/docs/designDoc/UDF/wordStats/index.html)).
- **Files**: LEXICON (unit records, 56 MB); **LRAGR** (inflections, 121.7
  MB: `EUI|inflected form|category|inflection code|base|citation form`);
  **LRSPL** (spelling variants, 8.9 MB: `EUI|variant|base`); LRABR
  (abbreviations and acronyms, 20 MB), LRNOM (nominalizations, 35,712
  rows), LRWD (word index), and others; DM.DB, SM.DB, AM.DB,
  inflVars.data (129 MB), XML and ASCII versions; LEX.tgz 1.06 GB in all
  (sizes from HTTP headers).
- **Checked in the 2026 LRAGR**: mice→mouse, foci/focuses→focus,
  children→child, bacteria→bacterium, analyses→analysis,
  leaves/leafs→leaf; the `glreg` code handles fimbria→fimbriae and
  gingivitis→gingivitides.
- **Checked in LRSPL**: 180,414 rows over 121,753 entries —
  oesophagus/esophagus, anaemia/anemia, labour/labor, sulphur/sulfur,
  grey/gray, programme/program, haemoglobin/hemoglobin, foetus/fetus;
  49,059 single-token rows differ only by a hyphen (`co-activate`,
  `coactivate`). Gap: single-word *aluminium* is not a variant of
  *aluminum* (only inside multi-word terms).
- **Cadence**: yearly since 1994; 2026 is the latest published (files
  2025-12-17; [LVG](https://lhncbc.nlm.nih.gov/LSG/Projects/lvg/current/web/index.html)
  2026 on 2025-12-18); the home page already names a "2027 Release", so
  the next is likely due around December 2026 (inferred).
- **Tools**: LVG 2026 is 588 MB full, 324 MB lite (Java,
  [download](https://lhncbc.nlm.nih.gov/LSG/Projects/lvg/current/web/download.html)).
- **Licence**: no UMLS licence needed (only the Metathesaurus needs one).
  The [SPECIALIST NLP Tools
  Terms](https://lhncbc.nlm.nih.gov/LSG/Projects/lexicon/current/web/termsAndConditions.html):
  free to all; redistributions include the conditions; any publication
  or distribution cites "the SPECIALIST NLP Tools with the release number
  and date" and states modifications; no proprietary claim, no
  presenting it as other than a US government product, no use of NLM's
  name to endorse. No clause against commercial use (a reading, not legal
  advice).
- **How LVG normalizes**:
  [`norm`](https://lhncbc.nlm.nih.gov/LSG/Projects/lvg/current/docs/userDoc/tools/norm.html)
  builds UMLS's normalized indexes — genitives, punctuation and stop
  words removed, lowercased, each word to its base form, spelling
  variants to the citation form ("colouring" → "color"), folded to ASCII,
  words sorted — and gives **several** outputs for an ambiguous form
  ("scleroses" → sclerosis or sclerose).
  [`luiNorm`](https://lhncbc.nlm.nih.gov/LSG/Projects/lvg/current/docs/userDoc/tools/luiNorm.html)
  always gives **one**, by "canonical classes", and NLM warns "the
  canonicalized form of 'left' is 'leaf', an unrelated word": not a model
  to copy. History in [Lu et al., JAMIA
  2020](https://academic.oup.com/jamia/article/27/10/1600/5848743).

## Other languages

Broad and share-alike: Wiktextract (CC BY-SA 4.0 + GFDL), UniMorph and
MorphyNet (CC BY-SA 3.0). Language by language, permissive: **Japanese,
SudachiDict** (Apache-2.0, updated 2026-09), whose "normalized form" is a
spelling-variant table — okurigana (打込む→打ち込む), script
(かつ丼→カツ丼), variant kanji (附属→付属), misspellings
(シュミレーション→シミュレーション) ([Sudachi](https://github.com/WorksApplications/Sudachi)),
with a Rust implementation, sudachi.rs (Apache-2.0); **Chinese, OpenCC**
traditional↔simplified (Apache-2.0, v1.4.2, 2026-08), with Rust ports
`ferrous-opencc` and `opencc-rust` (Apache-2.0; the `zhconv` crate is
GPL-2.0-or-later). Other licences: Russian, OpenCorpora (CC BY-SA 3.0,
per [pymorphy2](https://github.com/pymorphy2/pymorphy2)); French, Lefff
(LGPL-LR, about 110K lemmas, per Wikipedia, not checked at the source).
Not researched: German old/new spelling (daß/dass), Portuguese
pt-PT/pt-BR.

## Use in retrieval

- **PubMed** documents its practice: the Subject Translation Table holds
  "British and American spellings", "singular and plural word forms",
  and "Terms derived from the [UMLS] that have equivalent synonyms or
  lexical variants in English"; phrase searches "do not otherwise
  include alternate spellings or singular/plural forms" ([PubMed
  Help](https://pubmed.ncbi.nlm.nih.gov/help/), read 2026-09-29; British
  MeSH spellings added 2003, [Tech
  Bull](https://www.nlm.nih.gov/pubs/techbull/ma03/ma03_british.html)).
  No downloadable copy of the table was found.
- **Krovetz 1993**: the dictionary-checked inflectional stemmer beat no
  stemming by +9.7% (CACM), +33.4% (NPL), +1.8% (TIME), +4.1% (WEST);
  Porter misses matrices/matrix; NPL's queries said "transistorised"
  where the documents said "transistorized"; irregular plurals are tied
  to senses — *antennae* for insects, *media* in the entertainment sense
  ([PDF](http://people.scs.carleton.ca/~armyunis/projects/KAPI/Krovetz.pdf)).
- Hull 1996: modest average stemming gains ([ACM
  survey](https://dl.acm.org/doi/10.1145/2975608)); Trieschnigg et al.
  2007: biomedical retrieval sensitive to tokenization, stemming helping
  some queries ([Twente](https://research.utwente.nl/en/publications/the-influence-of-basic-tokenization-on-biomedical-document-retrie/));
  LVG evaluated for concept mapping, not ranked retrieval ([Divita
  1998](https://pubmed.ncbi.nlm.nih.gov/9929324/)). No controlled study
  of British/American normalization in ranked retrieval was found.
- **Elasticsearch**: algorithmic stemmers miss mouse/mice and foot/feet;
  "a dictionary stemmer is only as good as its dictionary";
  `stemmer_override` "must be placed before any stemming filters"
  ([stemming](https://www.elastic.co/docs/manage-data/data-store/text-analysis/stemming),
  [override](https://www.elastic.co/docs/reference/text-analysis/analysis-stemmer-override-tokenfilter)).
- **KStem gains little here**: Lucene's KStem has a 40-pair
  `directConflations` list (going→go and the like) and none of mice,
  children, foci or nuclei
  ([source](https://github.com/apache/lucene/blob/main/lucene/analysis/common/src/java/org/apache/lucene/analysis/en/KStemmer.java)).
- **Pitfalls, confirmed**: WordNet maps *axes* to ax and axis, *leaves*
  to leaf and leave, *left* to leave. VarCon marks variants valid in one
  sense only — `check | verify` against `check / cheque | bank`; `tire |
  exhausted` against `tire / tyre | wheel`; `story | book` against
  `story / storey`; `meter | measuring device` against `meter / metre` —
  so cheque→check is safe and check→cheque is not. SPECIALIST's base form
  points both ways: in single-word -ize/-ise pairs the -ise form is the
  base in 1,406 cases and the -ize in 5 (randomize→randomise); in 1,957
  ae/e pairs the US form is. The canonical form is ours to choose.

## Rust

No crate ships SPECIALIST. Related: `varcon` (crate-ci/typos; MIT/Apache
code) bundles VarCon 2020.12.07; `wordnet-lemmatizer` (Apache-2.0) has
morphy's data built in; `english` (MIT/Apache) is built from Wiktionary
data of unverified licence; `unimorph-core` reads UniMorph. Simplest
route: parse LRAGR and LRSPL (pipe-delimited; single-word rows) with
varcon.txt offline, write a sorted TSV of form → canonical, and build an
`fst::Map` (Tantivy already uses the `tantivy-fst` fork) or a
`phf_codegen` map, applied in a custom Tantivy token filter. Single-word
base and variant forms number 488,340; LRAGR's single-word rows were not
counted (the file was not downloaded whole). Hyphen variants work only
if applied before the tokenizer splits on hyphens.

## The agent's recommendation

- **English now**: SPECIALIST LRAGR + LRSPL as the core, VarCon/ESDB to
  fill gaps (aluminium, -ised inflections) and supply the sense flags.
  Choose the canonical form (e.g. VarCon's "A", American); drop, or emit
  both lemmas for, forms with more than one (leaves, axes, left, media,
  scleroses) — never collapse them as luiNorm does; skip variants flagged
  as one-sense-only.
- **Other languages later**: Wiktextract first, Sudachi for Japanese,
  OpenCC for Chinese.
- **Gold standard for stemmer collisions**: SPECIALIST's LRAGR rows
  grouped by entry id, with LRSPL for spelling groups — independent of
  any stemmer. Decide whether Paice's groups include derivations (then
  DM.DB, [Paice 1994](https://doi.org/10.1007/978-1-4471-2099-5_5)). Once
  the table is also the normalizer it can judge stemmers but not itself;
  retrieval gains still need relevance judgements.
- **Licence cautions**: share-alike sources (Wiktionary, UniMorph,
  MorphyNet, OpenCorpora) make any table distributed from them CC BY-SA
  — whether one kept on a server alone is distributed is a question for
  counsel; SPECIALIST wants its release number and date, modifications
  stated, and no implied NLM endorsement; ESDB's notices kept; CELEX and
  BioLexicon are paid, the UMLS Metathesaurus needs a licence.
