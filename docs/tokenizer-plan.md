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
  ceiling. *Recommended.*
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
  *Recommended.*
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
  its own treatment.*
- **D9. No stop list.** IDF already discounts `the`; a list turns
  `vitamin A` into `vitamin` (text analysis §3). *Recommended.*
- **D10. The reviewed variant table** (ideas 2 and 3): spelling variants
  mined from the term dictionary and from the titles two sources give one
  work, reviewed at the head and versioned. *Recommended: after D8, once
  the miss analysis says how much of what is left it would reach.*
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
  evaluation plan §14 (its E6). Tests: generated text for the analyzer's
  own properties (any text: normalization done twice changes nothing,
  the query side and the index side agree, no term past the cap, text
  with letters never yields no terms), and real titles in each script as
  fixed cases.
- **T2. The first treatment: D2 to D6**, each a switch, screened by a
  fractional factorial design on the screening index against a sample of
  RELISH's working half (§14.4), the survivors rebuilt over the corpus and
  ablated, confirmed on the whole working halves; D11's checks for the
  scripts no set measures.
- **T3. Stemming as a second field (D8)**, the same way.
- **T4. The variant table (D10)**, if T0 and T3 leave it enough to reach.
- **T5. Publish.** What is kept becomes the mapping: a new search version,
  a new epoch; the test halves and the held-out sets read at that
  milestone (evaluation plan §7).

## 6. What it costs

Measured so far: a search pass over the evaluation suites, 4 to 29 seconds
for a TREC year and about an hour for RELISH's two runs (evaluation plan
§13.5); epoch 7's whole publish, 4 hours 29 minutes, of which the lexical
index is a part. Not yet measured, and measured in T1: the lexical index
built alone over epoch 7, its size on the serving claim, and a treatment
index's size beside it.

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
   would make `µg` into `mug`. Found building T0: on the sample store,
   7,333 such runs in 3,515 of 110,469 works. The census counts them
   apart from the other Greek letters; D5 decides on epoch 7's numbers —
   `micro`, the name only when alone, or nothing.

## 9. Decision log

- **2026-09-29.** Every recommendation of §4 taken — the user: "go with
  your recommendations" — including the new dependency D2 names, ICU4X's
  `icu_segmenter`, for Thai, Lao, Khmer and Burmese and as the treatment
  that tries dictionary word breaking against bigrams for Chinese,
  Japanese and Korean. T0 started the same day; its commands built and
  run on a sample store the same day, the box's run prepared (§5).
