# Tokenizer plan

**Status:** draft 1, 2026-09-29. The decisions of §4 taken as recommended
(§9); T0 under way.
**Inputs:** the user's thesis and ideas in
[text analysis](inbox/text-analysis.md); the baseline every change is
measured against, [evaluation plan](evaluation-plan.md) §13, and how many
changes are tested, its §14; what the index leaves open,
[serving plan](serving-plan.md) §11 item 11; the analyzer's output on
text in several scripts, run 2026-09-29 (§3).

## 1. What this is for

The user's thesis (text analysis §1): "Stemming, lemmatization,
tokenization and stop words are a leading cause of bad BM25 retrieval
scores in scientific search", and "it is a finite and totally solvable
problem". On 2026-09-29, with the first measurement in hand: "the
tokenizer is first place to start looking."

Analysis is what turns a work's text, and a query's, into the terms the
index matches: where words are split, what is lowercased, folded, joined
or dropped. Every change here is judged against the baseline of
evaluation plan §13 — RELISH's working half fine enough to see a point or
two, TREC Precision Medicine 2019 the coarse human check — and tested as
§14 describes, since each is a rebuild of the lexical index.

The corpus is every language and every field it holds (epoch 7, PubMed
and Crossref), not English and not biomedicine alone: a language the
published sets cannot measure is still in scope, and is checked another
way (§4, D11).

## 2. What analysis must guarantee

Stated as invariants, measured over the whole corpus before anything is
fixed, and fixed a class at a time with a test that pins the census —
never a list of the cases seen so far.

- **I1. Every word a reader sees is findable.** No work's text yields no
  terms, and every word — as its language divides words — becomes at
  least one term.
- **I2. The query is analyzed as the text was.** A word typed into the
  box meets the same word in a title. (It holds today: the query side
  splits with each field's own analyzer, `search::translate`.)
- **I3. Spellings a reader takes for one word meet**: with or without its
  accents (`étude`, `etude`), a Greek letter or its name (`TNF-α`,
  `TNF-alpha`), hyphenated or joined (`IL-6`, `IL6`), a compatibility form
  or its plain one (full-width `ＤＮＡ`, `DNA`).
- **I4. What a reader takes for different words stays apart**: `ALL` the
  leukemia and `all`; `vitamin A` and `vitamin`; `C++` and `C`; a
  stemmer's collisions (`experiment`, `experience`).
- **I5. Nothing typed is dropped in silence** — no stop list, no length
  past which a term vanishes.

## 3. What the analyzer does today

Every text field — title, abstract, authors, venue — goes through
Tantivy's `default` analyzer (`crates/search/src/schema.rs`): a term is a
maximal run of letters and digits (`SimpleTokenizer`, which splits on
`!char::is_alphanumeric`), terms of 40 bytes or more are dropped
(`RemoveLongFilter::limit(40)`), the rest lowercased; no stemming, no stop
list (read in `tantivy-0.26.2/src/tokenizer/`).

Text in several scripts through it, run 2026-09-29:

| text | terms | invariant broken |
|---|---|---|
| `新型冠状病毒肺炎患者的临床特征分析` (a Chinese title) | **none** | I1: one 17-character run, 51 bytes, over the cap |
| `日本における高齢者の認知症に関する疫学研究` (Japanese) | **none** | I1, the same way |
| `การศึกษาความชุกของโรคเบาหวานในประเทศไทย` (Thai) | **none** | I1, the same way |
| `肺癌 基因 治疗` (short Chinese, spaced) | `肺癌` `基因` `治疗` | I1 in general: an unspaced run is one term, so no word inside it is findable even under the cap |
| `한국인 성인의 비만과 …` (Korean) | `한국인` `성인의` `비만과` … | I1 partly: a particle stays on its word (`성인의` is `성인` and `의`) |
| `دراسة وبائية لمرض …` (Arabic) | `لمرض` … | the same: a clitic stays on its word |
| `pneumonoultramicroscopicsilicovolcanoconiosis` | **none** | I5: 45 bytes |
| `… methylenedioxymethamphetamine` | kept (29 bytes) | a longer systematic name would not be |
| `Étude de l'efficacité` | `étude` `de` `l` `efficacité` | I3: `etude` finds nothing |
| `TNF-α and β-lactamase, δ13C` | `tnf` `α` `β` `lactamase` `δ13c` | I3: `alpha` finds nothing |

The text analysis note's §3 has the rest for English scientific text:
`IL-6`, `IL6` and `interleukin 6` meeting only on `6`; `C++` becoming `c`;
`tumour` and `tumor` apart; what a stemmer and a stop list would add.

So whole languages — Chinese, Japanese, Thai — are close to unsearchable
today. How many works that is in epoch 7 is the first thing to count
(§5, T0), not a figure guessed here.

### 3.1 Over epoch 7 (T0's census, 2026-09-29)

`survey analysis` over epoch 7: 192,227,584 rows, 188,718,279 held works
with text, their titles and abstracts (Job `ol-rht-tokenizer-t0-v1`;
report under `evaluation/tokenizer/t0/` on the bulk claim).

**I1, by the title's script** — the language a record gives does not
say (125,036 works tagged English have titles that yield no terms, most
of them in Han), so the script does:

| title's script | works | title yields no terms | no title or abstract term at all |
|---|---|---|---|
| Han | 367,592 | 183,632 (50.0%) | 141,719 (38.6%) |
| Katakana | 16,748 | 4,551 (27.2%) | 4,429 |
| Hiragana | 5,964 | 1,516 (25.4%) | 1,392 |
| Thai | 71,972 | 4,289 (6.0%) | 1,351 |
| Latin | 177,547,855 | 1,057 | 947 |
| Cyrillic | 855,709 | 212 | 81 |

In all, 195,280 titles yield no terms (0.109% of titled works) and
149,935 works have no term from title or abstract. And where a run of
those scripts is under the cap it is indexed whole, a clause as one term,
findable only by the whole clause:

| script | runs | one term each | dropped by the cap | mean characters |
|---|---|---|---|---|
| Han | 1,718,223 | 1,045,525 | 672,698 | 11.9 |
| Thai | 10,071,750 | 8,109,615 | 1,962,135 | 8.5 |
| Katakana | 42,722 | 30,251 | 12,471 | 11.4 |
| Hiragana | 39,135 | 25,308 | 13,827 | 12.3 |
| Lao | 35,058 | 30,069 | 4,989 | 7.0 |
| Myanmar | 12,092 | 11,712 | 380 | 4.9 |
| Khmer | 1,312 | 1,260 | 52 | 4.7 |

**I5 and D3, the cap.** It counts bytes, so it bites scripts of two bytes
a letter at 20 letters: 151,879 Cyrillic runs dropped, in 74,656 of the
855,709 Cyrillic-titled works (8.7%), and 13,409 Greek runs (4,953 of
88,153 Greek-titled works, 5.6%). Of Latin, 1,011,351 runs: at 40 to 63
bytes real systematic names (`trifluoromethoxycarbonylcyanidephenylhydrazone`),
at 64 to 127 protein sequences, past that nucleotide sequences, runs of
accession numbers, hashes, base64, and tables whose spaces were lost.

**I3, how often the spellings occur** (runs, and works with one):
accented Latin letters 145,482,966 in 14,231,706 works (7.5%);
compatibility forms (`α₄`, `I²`, `¹H`) 753,062 in 314,708; a Greek letter
alone 6,962,575 in 2,707,908; other Greek letters joined to letters or
digits 4,742,227 in 1,592,785; **mu or the micro sign joined (`μg`,
`μM`) 2,491,175 in 1,215,954** (§8 item 4); Greek words 15,661,327 in
157,123; pairs joined by a hyphen 347,795,692 in 78,360,362 (41.5%);
letters beside digits or another script 114,122,440 in 26,708,520
(14.2%). **I4**: short all-capital words 342,600,727 in 58,483,244 works
(31.0%).

### 3.2 What it cost the first measurement (T0's miss analysis)

`evaluate misses` over the working sets and the baseline's runs (the
restricted runs here; the open ones agree within a point). A pair is a
query and a relevant work the run missed; *gaining* means the work's text
would hold at least one more of the query's words under the change — a
ceiling, not a gain (§7 item 4).

| change | TREC PM 2019, missed pairs gaining | RELISH, missed pairs gaining | judged not relevant in the top 10, gaining (TREC / RELISH) |
|---|---|---|---|
| English stem (D8) | 12.0% | 51.4% | 2.9% / 27.6% |
| joins (D6) | 6.6% | 2.5% | 0% / 1.3% |
| Greek names (D5) | 0% | 0.4% | 0% / 0% |
| folding (D4) | 0% | 0.1% | 0% / 0% |
| cap lifted (D3) | 0% | 0% | 0% / 0% |
| all of them | 17.3% | 52.9% | 2.9% / 28.6% |

Over 2,790 TREC pairs and 34,288 RELISH pairs. Stemming is the one large
lever on English, and it gains in the relevant works missed four times
(TREC) and twice (RELISH) as often as in the wrong works found. Folding,
Greek names and the cap barely touch English biomedical queries; their
case is §3.1's. What no change of form reaches — 83% of TREC's missed
pairs, 47% of RELISH's — is, for TREC, mutation codes the relevant works
do not write (`D770N`, `G12V`), other names (`cholangiocarcinoma`),
`burden` (the works' `TMB`) and `tumor` beside `tumour` (D10, D12); for
RELISH, the title's own filler (`from`, `analysis`, `novel`), the title
as query (evaluation plan §12).

## 4. Decisions

Recommendations; the user's to take or change. The indexing policy is the
user's (serving plan §11 item 11), and this plan is its analysis part.

- **D1. Measure before changing (T0).** A census of epoch 7 against each
  invariant, and a miss analysis of the baseline's working sets. Nothing
  rebuilt for it. *Recommended.*
- **D2. Scripts written without spaces.** Two ways, not exclusive:
  (a) **bigrams** for Han, Hiragana, Katakana and Hangul runs — every two
  adjacent characters a term, as Lucene's CJK analyzer does: no
  dictionary, no guessing the language, new names found as readily as
  old words; loose, since a bigram can straddle two words;
  (b) **dictionary word breaking** by Unicode's rules with the ICU
  project's data (the `icu_segmenter` crate, ICU4X): real words for
  Chinese, Japanese and Thai, Lao, Khmer and Burmese, which have no
  bigram answer; a new dependency with its own data, named here before it
  lands, and a word the dictionary lacks is split wrongly.
  *Recommended: (a) for the four CJK scripts in the first treatment, (b)
  for Thai and its neighbours, and (b) for CJK as its own treatment
  against (a) — the measurement decides between them.*
- **D3. The length cap.** Raise it to what the longest real terms need,
  as the census finds them (T0), with the engine's own limit the only
  ceiling. *Recommended.* **Taken, after T0**: no cap on titles or
  abstracts, the engine's own limit the only ceiling — the cap counted
  bytes and dropped ordinary Russian and Greek words (§3.1).
- **D4. Compatibility forms and accents.** Unicode normalization (NFKC:
  full-width to plain, ligatures apart, superscripts to digits) and
  folding of diacritics (`é` to `e`), in the index and the query alike.
  Folding joins words some languages keep apart (`résumé`, `resume`);
  keeping the exact form in a second field fixes that at the cost of the
  index's size. *Recommended: fold in the first treatment; a second field
  only if the measurement shows folding costs precision.*
- **D5. Greek letters.** A Greek letter standing alone or joined to Latin
  letters or digits (`TNF-α`, `δ13C`, `β-lactamase`) also yields its name
  (`alpha`, `delta`, `beta`); a run of Greek text — a Greek-language
  title — stays Greek, since spelling out its letters would destroy it.
  *Recommended.* **Mu in units, taken after T0**: mu or the micro sign
  joined to letters or digits (`µg`, `μM`, 1.2 million works) also
  yields its `u` form (`ug`, `um`), as units are typed without the Greek
  letter; mu alone yields `mu` like any other letter.
- **D6. Hyphens and joins.** A word split at a hyphen or between letters
  and digits yields its parts and the joined whole (`IL-6` gives `il`,
  `6` and `il6`; `TNF-α` gives `tnfα` beside the rest), positioned so a
  phrase still matches (Lucene's word delimiter graph does this).
  *Recommended.*
- **D7. Case.** Lowercasing stays; protecting short all-capital words
  (`ALL`, `CAT`) as distinct terms (text analysis §6 idea 5) is its own
  later treatment, measured, since it also splits `DNA` from `dna`.
  *Recommended: later.*
- **D8. Stemming**, when it comes, in a second field beside the
  unstemmed one, the unstemmed weighted higher so a stemmer's collision
  cannot outrank an exact match (idea 4), and by each work's language —
  an English stemmer for English, Snowball's for the languages it has —
  which the merged record already carries. *Recommended: after D2–D6, as
  its own treatment.* **Changed after T0**: English stemming joins T2's
  screen as one more switch, since the miss analysis gives it the
  largest ceiling on the English sets (§3.2); the other languages'
  stemmers stay T3. And the merged record's language cannot choose the
  texts — a third of works have none, and some have the wrong one
  (§3.1) — so D13 identifies them. Which English stemmer, what it leaves
  alone, and how collisions are counted before a rebuild: a survey under
  way (2026-09-29), then candidates screened as levels of this factor.
- **D9. No stop list.** IDF already discounts `the`; a list turns
  `vitamin A` into `vitamin` (text analysis §3). *Recommended.*
- **D10. The reviewed variant table** (ideas 2 and 3): spelling variants
  mined from the term dictionary and from the titles two sources give one
  work, reviewed at the head and versioned. *Recommended: after D8, once
  the miss analysis says how much of what is left it would reach.*
  **Brought forward, after T0 — a lexicon table first** (taken
  2026-09-29): irregular inflections (`mice`, `foci`, `bacteria`,
  `analyses`) and British and American spellings (`labour`, `tumour`,
  `oesophagus`) are lookups no stemmer makes, so a table from published
  lexicons maps each English word form to one canonical form, applied
  alike to text and query in the stemmed field, before the stemmer; its
  own switch in T2's screen, apart from the stemmer so each is measured
  alone. Built from NLM's SPECIALIST Lexicon (its inflection and spelling
  tables, LRAGR and LRSPL; open, cited by release number and date, our
  changes stated) and VarCon (general English spellings, and its flags on
  variants that hold in one sense only, which are skipped); the canonical
  form American; a form with two lemmas (`leaves`, `axes`, `left`,
  `media`) mapped to both, never collapsed to one. Contextual
  lemmatization stays out. Mining the term dictionary is what is left of
  D10 afterwards. The survey: `docs/inbox/word-form-lexicons.md`. The
  same inflection groups serve as the collision census's key for the
  stemmers, independent of any stemmer.
- **D11. Measuring what the published sets cannot.** Every set so far is
  English, so D2's effect on Chinese, Japanese and Thai shows in none of
  them. What checks it: the census of I1 run again after the change (the
  works whose text now yields terms, by language), explicit cases from
  real titles in each script, and, when their hurdles are cleared, judged
  sets in those languages (NTCIR's Japanese academic collections, the
  CLEF Domain-Specific track's German and Russian — the benchmarks note
  §2.4). *Recommended.*
- **D12. Not here: synonyms and abbreviations.** `heart attack` and
  `myocardial infarction`, `MI`: they depend on context and belong to the
  query, not the index (idea 6) — a query-time change, cheap to test
  (evaluation plan §14.1), its own plan after this one.
- **D13. Which language a text is in** (added after T0). Identified per
  text — a title and an abstract apart, since a work can have an English
  title and an abstract in another language — when the lexical index is
  built, not in the model or the merge: it is analysis, and it changes
  the search version alone. Each text gets a language and a confidence;
  below a floor it gets no stemmed form and searches as it does today, so
  a wrong or unsure call costs only the stemmed field. The detector:
  lingua-rs, provisionally, pending a survey of what exists (2026-09-29:
  the Rust crates, the fastText-derived models and whatever has overtaken
  them), judged on accuracy on short academic titles, the languages
  covered, the licence of code and model, speed and memory on the box, and
  pure Rust against a C++ build dependency. Its key, independent of any
  detector and of the records' tags: PubMed's titles, which NLM
  translates into English, beside their original-language titles
  (`VernacularTitle`, kept in the normalized layer), tagged by NLM's
  indexers; and a hand-checked sample of Crossref's titles by script.
  *Built 2026-09-29 for the measurement:* `ol-rht evaluate language-key`
  (the key: NLM's English titles and bracketed translations, original
  titles and other-language abstracts under NLM's tags, English
  abstracts, at most 1,000 texts a stratum of kind, language and length,
  drawn by a seeded hash with each stratum's population kept; and
  Crossref's titles by script, unlabelled); `ol-rht-language detect`, a
  binary of its own so no other carries a detector's models (lingua, all
  75 languages in the image, and GlotLID through a pure-Rust fastText
  reader); `ol-rht evaluate language-score` (English called English by
  kind and length, each language and script answered right and called
  English, calibration, English at each confidence alone and with both
  detectors agreeing, languages folded to ISO 639-3 macrolanguages by
  SIL's mappings, intervals as the evaluation's). The survey of detectors
  is `docs/inbox/language-identification.md`; its run on the box, the
  Jobs `ol-rht-fetch-models-v1` and `ol-rht-language-d13-v1`.

## 5. Order

- **T0. Census and miss analysis** — no rebuild.
  - *Census of epoch 7*, per language: works whose title or abstract
    yields no terms; terms dropped by the cap, and the lengths of the
    longest real terms; how often Greek letters, diacritics,
    compatibility forms and letter-digit joins occur — the size of each
    class of §2, measured.
  - *Miss analysis* of the baseline runs, on the working sets only (TREC
    Precision Medicine 2019, RELISH's working half; never the test halves
    or 2020): for every relevant work the search missed, whether a
    candidate change — folding, a Greek name, a join, a stem — would make
    the query's words appear in the work's text. That share, by change,
    is a ceiling on what each can gain (text analysis §6 idea 1); it
    needs no model, since it asks only whether the words would meet.
    What no change of form reaches — a synonym, a concept never named —
    is counted and left to D12.
  - *Built 2026-09-29*: `ol-rht survey analysis` (the census) and
    `ol-rht evaluate misses <suite> <run>` (the miss analysis), each
    change a function over terms in `crates/search/src/analysis.rs`, with
    a property test that the census's runs under the cap are exactly the
    terms Tantivy makes. Run on a throwaway sample store; on the box by
    the homelab Job `ol-rht-tokenizer-t0-v1`, over epoch 7 and the
    baseline's runs.
- **T1. The machinery.** The analyzer as a module of `crates/search`,
  registered with Tantivy under a versioned name, each change a switch;
  the mapping's description naming it (every report prints it); a
  lexical index built beside the epoch's and never published; `evaluate
  run` pointed at it; the screening index and sampled suites of
  evaluation plan §14 (its E6); the language detector (D13), measured
  against its key before it chooses anything, and the English stemmed
  field (D8). Tests: generated text for the analyzer's own properties
  (any text: normalization done twice changes nothing, the query side and
  the index side agree, text with letters never yields no terms), and
  real titles in each script as fixed cases.
- **T2. The first treatment: D2 to D6 and English stemming (D8)**, each
  a switch, screened by a fractional factorial design on the screening
  index against a sample of RELISH's working half (§14.4), the survivors
  rebuilt over the corpus and ablated, confirmed on the whole working
  halves; D11's checks for the scripts no set measures.
- **T3. Stemming for the other languages (D8)**, by D13's languages, the
  same way.
- **T4. The variant table (D10)**, if T0 and T3 leave it enough to reach.
- **T5. Publish.** What is kept becomes the mapping: a new search version,
  a new epoch; the test halves and the held-out sets read at that
  milestone (evaluation plan §7).

## 6. What it costs

Measured so far: a search pass over the evaluation suites, 4 to 29 seconds
for a TREC year and about an hour for RELISH's two runs (evaluation plan
§13.5); epoch 7's whole publish, 4 hours 29 minutes, of which the lexical
index is a part. T0 on the box: the census 667 seconds over epoch 7's
192 million rows (a reader and four threads, 438 MiB at its peak, the
artifact mapped); the miss analysis 4 seconds a TREC run, 49 a RELISH
run. Not yet measured, and measured in T1: the lexical index built alone
over epoch 7, its size on the serving claim, and a treatment index's size
beside it.

## 7. What it compromises

Kept as the evaluation plan keeps its own (§12 there): each compromise,
what it costs, what would lift it, entered as it is made. Foreseen:
bigrams find strings that straddle two words (D2); folding joins words a
language keeps apart (D4); the first treatments are measured on English
sets alone, the other scripts by census and cases (D11).

Made in T0:

1. **Scripts by Unicode block**, not by the Unicode Script property: a
   block names a script for most letters, and a letter outside the
   blocks named is counted as none. *Costs* a few letters misfiled
   (Latin Extended-C, say). *Lifted by* ICU4X's script property once D2
   brings the dependency.
2. **The census reads titles and abstracts**, not authors or venues,
   which want their own analysis (§8 item 2). *Costs* nothing measured
   of names. *Lifted by* that item.
3. **A query's words, in the miss analysis, are its text as the
   analyzer splits it**, not the parser's tree: a character the parser
   reads as syntax (a parenthesis in TREC's `(V600E)`) is split as the
   analyzer splits it. *Costs* a word counted that the search may have
   read otherwise. *Lifted by* taking the words from the parsed query.
4. **Relevant is lenient** (any grade above 0), and **meeting is not
   ranking**: a word that would meet a work is a ceiling on what the
   change gains, not a gain. The judged non-relevant top 10 is counted
   beside it for that reason. *Lifted by* T1's rebuilt index, measured.
5. **One stemmer, English, for every work** in the miss analysis (D8
   wants each work's language). The sets are English. *Lifted by* T3.
6. **The census's examples are the first works by id**, not a sample:
   deterministic, and biased toward early ids (PubMed's first). *Lifted
   by* a seeded sample of examples, if the first ones mislead.

Made in measuring D13:

7. **The key's labels are NLM's, text and all.** Some original titles are
   romanized (Russian in Latin letters) and some older ones have lost
   their accents (`Hygiena zeny`): the label is right, the text is not the
   language as its readers write it. *Costs* a detector's score on those
   strata; the score reads each language by its text's script, so
   `rus/Latin` and `rus/Cyrillic` stand apart. *Lifted by* keys from
   other sources.
8. **The key is PubMed's**: biomedical, weighted toward European
   languages; Crossref's titles, where the untagged third of the corpus
   lives, are unlabelled until a person reads the hand-check sheet.
   *Costs* the languages PubMed rarely has. *Lifted by* that sheet, and
   title-length slices of FLORES+ and UDHR-LID.
9. **PubMed-weighted English precision has no interval**: it is a ratio,
   and the evaluation's bootstrap bounds means. *Costs* a point estimate
   read without its uncertainty (the per-language shares called English,
   which have intervals, carry the same news). *Lifted by* a bootstrap of
   the ratio.
10. **langram is left out of the first round**: its models are a separate
    581 MB download, it has one maintainer, and its only evaluation is its
    author's, on data it may have been trained on. *Costs* the one fast,
    pure-Rust detector of 188 languages. *Lifted by* adding it if lingua
    and GlotLID both disappoint.
11. **GlotLID is read through a vendored `fasttext-pure-rs`** whose guard
    against malformed files is raised from 1 GiB to 4 GiB: the published
    guard refused GlotLID's 1.67 GB input matrix on the box
    (`ol-rht-language-d13-v1`, 2026-09-30), and the crate has no way to
    set it. One constant changed, in `vendor/fasttext-pure-rs/`, put in
    place by the workspace's `[patch.crates-io]`; answers on lid.176 the
    same as the published crate's. *Costs* a copy of a third-party crate to
    keep. *Lifted by* an upstream release that lets a caller set the limit.

## 8. Open items

1. Whether T0's miss analysis should also sort what no change of form
   reaches (a synonym, an unnamed concept) with a model's help, checked
   against a person's reading of a sample; counted plainly first.
2. Authors and venues want their own analysis (text analysis §7 item 3):
   names folded and transliterated (`Mueller`), venues their
   abbreviations (`J Biol Chem`). After titles and abstracts.
3. Where the switches live once T5 picks: a switch left in the code for a
   change not kept is dead weight; the plan's record of it is enough.
4. **Mu in units.** The micro sign and mu joined to letters (`µg`,
   `μmol`) are units far more often than the letter, and D5 as written
   would make `µg` into `mug`. Found building T0; over epoch 7, 2,491,175
   such runs in 1,215,954 works (§3.1). Settled: the `u` form (D5).
5. **Found by T0, outside this plan.** 8,907,022 works (4.7%) have no
   title or abstract with a letter or digit, findable by authors and
   venue alone. And 57,414 works are tagged Ndonga (`ndo`), with text
   that reads as English; `ng` is Ndonga's two-letter code and also
   Nigeria's country code, so a country read as a language is likely,
   not yet checked against the records. Kanuri (`kr`, Korea's country
   code) and Zhuang (`za`, South Africa's) are tagged on a few works too.
   Both are the model's and the merge's to look at.

## 9. Decision log

- **2026-09-29.** Every recommendation of §4 taken — the user: "go with
  your recommendations" — including the new dependency D2 names, ICU4X's
  `icu_segmenter`, for Thai, Lao, Khmer and Burmese and as the treatment
  that tries dictionary word breaking against bigrams for Chinese,
  Japanese and Korean. T0 started the same day; its commands built and
  run on a sample store the same day, the box's run prepared (§5).
- **2026-09-29, after T0** (§3.1, §3.2). The user: "ok to drop length
  limit on titles", then "yes drop the limit on abstracts too" — D3, no
  cap. D5's `u` form for mu in units: "yes". D8: "ok to add stemming for
  english", into T2's screen; the user asked what exactly English
  stemming will be, and how English is told — D13 added, lingua-rs
  provisionally "pending the outcome of a survey of what's out there
  today, particularly in rust, but also what exactly fastText
  derivatives are up to these days/if their crown has been taken by
  another method". Both surveys (detectors, English stemming) under way.
- **2026-09-29, after the surveys.** The detector measurement: the user,
  "i'd say yes to that, but we keep running out of disk space on my
  laptop; might need to do this experiment on the cluster" — built so,
  lingua and GlotLID measured on the box (langram left out, §7 item 10).
  English stemming: "kstem is a good idea; the others i wouldn't bother
  with. *maybe* current porter2" — the factor's levels none, KStem and
  perhaps current Porter2. The user asked about irregular plurals and
  British and American spellings, and "do we want to keep lemmatization
  out of this for now?" — recommended, not yet decided: a lookup table
  from a published lexicon as its own switch, contextual lemmatization
  kept out; a survey of lexicons under way.
- **2026-09-29, after the lexicon survey.** The user: "yes to the lexicon
  table as recommended, ambiguous forms mapped to both" — D10's lexicon
  table (SPECIALIST and VarCon, American canonical, one-sense variants
  skipped, contextual lemmatization out), a switch of its own in T2.
