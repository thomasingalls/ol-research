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

### 3.3 Which language a text is in (D13's measurement, 2026-09-30)

The Jobs `ol-rht-language-d13-v1` (the key, lingua) and `-v2` (all three
detectors on one build, main-fc21636): 143,557 texts drawn from 40.0
million PubMed and 179.5 million Crossref records, 132,599 of them
labelled by NLM (18,161 English), the rest Crossref titles by script.
Each detector's top answer, any confidence; shares with 95% intervals over
texts:

| | GlotLID v3 | lingua 1.8 | lingua, low accuracy | both English |
|---|---|---|---|---|
| English titles called English | 0.866 (0.859–0.872) | 0.718 (0.709–0.727) | 0.564 | 0.682 |
| English abstracts called English | 0.904 (0.898–0.911) | 0.909 (0.902–0.915) | 0.765 | 0.865 |
| other languages' texts called English | 0.015 (0.015–0.016) | 0.008 (0.008–0.009) | 0.010 | 0.006 |
| texts a second, 8 threads | 12,167 | 3,385 | 2,593 | — |
| memory; load | 3.4 GB; 2.8 s | 320 MB; built in | 330 MB; built in | — |

- **lingua takes scientific English for Latin**: its commonest wrong
  answer on English texts is `lat`, 2,269 of 18,143; GlotLID's is `nld`,
  333.
- **lingua names Latin for scripts it lacks**: of Crossref's titles,
  GlotLID calls Lao 190 of 191 and Burmese 68 of 70, and Khmer 30 of 66
  (36 undetermined); lingua calls most of them `lat`, since it must pick
  one of its 75 languages.
- **GlotLID answers where there is no language**: titles without a
  letter get a language (`pol`, `eng`), where lingua gives none; and it
  says undetermined for 112 of 4,333 Chinese titles (lingua: 0).
- **By language**, GlotLID is ahead on most of Europe's — Spanish,
  French, Italian, Portuguese, Hungarian, Dutch (0.879 against 0.800),
  Danish (0.807 against 0.666), Swedish, Slovenian — and lingua on German,
  Turkish, Serbo-Croatian, Icelandic and Chinese (0.999 against 0.940).
  Titles NLM romanized (Russian, Bulgarian, Ukrainian, Greek in Latin
  letters) are 0 for all (§7 item 7); texts in their own script (Korean,
  Cyrillic Russian, Arabic, Japanese kana) near 1 for all.
- **Short text is where all fail**: English titles of 20 characters or
  fewer, 0.666 (GlotLID) and 0.548 (lingua); from 51 characters, GlotLID
  0.93–0.97.
- **Confidence** tracks being right for GlotLID (0.22 right below 0.5,
  0.98 at 0.99 and above), less for lingua (0.95 at the top), little for
  lingua's low mode. A floor mostly costs English: GlotLID at 0.5 calls
  0.792 of English titles English, and 0.011 of the rest.
- **Both agreeing** halves the texts wrongly called English (0.015 to
  0.006) and costs 18 points of English titles.

PubMed-weighted precision is 0.9995–0.9999 for every detector: PubMed is
mostly English, so it says little (§7 item 9). The full report is on the
bulk claim under `evaluation/language/ol-rht-language-d13-v2/`, with the
hand-check sheet of Crossref's titles.

### 3.4 The rules D13 weighed (2026-09-30)

`ol-rht-language-d13-v3`, on key-v2: the same texts, each title with its
own work's abstract (43,044 abstracts answered beside titles), and
Crossref's titles drawn apart by whether they have one (19,808). GlotLID
answered 195,451 texts in 12.4 s on 8 threads.

**Weighted to how often each length occurs**, GlotLID's top answer calls
0.953 of English titles and 0.999 of English abstracts English (lingua:
0.801 and 1.000). §3.3's 0.866 counted a title of 20 characters or fewer
as much as one of 100; most titles are longer.

**GlotLID's misses on English titles, read** (the score's sample): the
short ones are words any language may be written in — `Discussion.`,
`Erratum.`, `BOTULISM.`, `Scabies.` — and the long ones plainly English
technical titles called Danish, Dutch, Slovak or Slovenian at low
confidence, 0.15 to 0.31: `Identification and validation of reference
genes for RT-qPCR analysis in fetal rat pancreas.` (Danish, 0.19),
`Toxicological safety assessment of iomeprol, a new X-ray contrast agent.`
(Slovak, 0.21), titles thick with chemical and species names. Among the
texts in other languages called English, several labels are wrong (§7
item 12): `In Process Citation.` tagged Greek, `Guidelines on AIDS and
first aid in the workplace.` tagged Hindi, an English sentence tagged
Catalan.

**English unless GlotLID names another language at a confidence t**:

| t | English titles (weighted) | English abstracts (weighted) | other languages' texts called English |
|---|---|---|---|
| — (top answer) | 0.866 (0.953) | 0.904 (0.999) | 0.015 |
| 0.5 | 0.947 (0.990) | 0.950 (1.000) | 0.155 |
| 0.7 | 0.975 (0.996) | 0.968 (1.000) | 0.250 |
| 0.9 | 0.991 (0.999) | 0.989 (1.000) | 0.366 |

The low-confidence misses above are what the rule takes back. The texts
in other languages it calls English are mostly the short, the
language-neutral and the mislabelled of the key's 96,378 original-language
titles; for English stemming each costs a stemmed copy in a secondary
field. lingua at 0.5: 0.895 (0.897) of English titles, and 0.277 of the
rest.

**A title borrowing its abstract's language** (the abstract's answer at
0.5 or above), over the 34,362 labelled titles with an abstract beside
them — 4,678 English, 29,684 in other languages, most of those beside an
English abstract:

| rule | English titles English | other titles English | other titles right |
|---|---|---|---|
| the title alone | 0.930 | 0.008 | 0.817 |
| title of 20 characters or fewer | 0.948 | 0.096 | 0.760 |
| title's answer below 0.5 | 0.978 | 0.127 | 0.798 |
| 50 characters or fewer | 0.965 | 0.304 | 0.587 |
| 50 or fewer, or below 0.5 | 0.992 | 0.382 | 0.578 |

Borrowing gains English titles less than the default does, and pulls
titles in another language to English where the abstract beside them is
English — which, in the index, is a Crossref title in its own language
over an English abstract; how often that happens is what the hand-check
sheet (on the claim, with each title's abstract and every answer) would
tell.

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
  real titles in each script, every word ICU4X finds in those titles
  split again alone (§7 item 13), and, when their hurdles are cleared, judged
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
  **Amended after the measurement (2026-09-30):** a record's language tag
  never decides a text's language. PubMed's `Language` is "the language
  in which the article was published" — the full text, one tag for each
  language it appears in — not the title and abstract the index holds:
  a Chinese article's PubMed title is NLM's English translation, and its
  abstract most often the English one its authors supplied. So every
  text is detected, the key labels only where the label settles the
  text's own language (as §3.3's key does), and two rules are measured
  as candidates: *English unless the detector names another language at a
  confidence t or above*, since an English text left unstemmed searches as
  today while another language stemmed as English costs little; and *a
  title borrowing its abstract's language* when the title is short or its
  answer unsure and the abstract's answer sure, since short titles are
  where every detector fails (§3.3). The borrowing is measured where it
  can go wrong too: a title in another language beside an English
  abstract, as PubMed's original-language titles sit beside theirs.
  **Decided 2026-09-30, on §3.4's numbers:** GlotLID v3, read through
  the vendored reader (§7 item 11); every title and abstract detected on
  its own, a record's tag never consulted; a text is English unless
  GlotLID names another language at a confidence of 0.5 or above — 0.5
  against 0.7 left a level of T2's screen, for retrieval to settle; no
  title borrows its abstract's language until labelled Crossref titles
  say how often a title and its abstract differ; each answer cached by
  the text's hash, so a rebuild detects only what is new. lingua is not
  used.
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
  real titles in each script as fixed cases. In steps (2026-09-30), so
  T2's screen of D2–D6 can start before stemming is built:
  - **T1.1 The analyzer.** One Tantivy tokenizer, D2–D6 each a switch:
    runs of letters and digits as today; a run of an unspaced script
    split into bigrams (Han, kana, Hangul) or ICU4X's word breaks (Thai,
    Lao, Khmer, Burmese, and CJK as the alternative); NFKC and folding;
    Greek letters' names and mu's `u`; hyphen joins and letter–digit and
    script splits as extra terms at the same position; the cap a switch
    (D3 decided it gone, kept switchable for the ablation). The switches
    a value, recorded in the index's manifest and spelled out in the name
    the tokenizer is registered under (`ol1-capnone-bigrams-fold1-…`), so
    a report shows what an index was built with. All switches off is
    today's analyzer, term for term — a property test holds it. Built
    2026-09-30 (`search::analyzer`): the index side writes a text's forms
    and every other spelling's at the same positions, the query side one
    canonical form of each piece, so a query never asks for every
    alternative at once. ICU4X's dictionary model, not its LSTM model:
    the LSTM reads a word by its neighbours, and `เบาหวาน` (diabetes),
    one word in a Thai title, is two typed alone — a query that misses
    its own title (I2; §7 item 13). Amended 2026-10-03, found writing
    T1.2's properties: a run is split by its stretches, not whole — a
    word glued to a script written without spaces (`IL-6基因多态性`,
    `血清IL6水平`) is still a word, with its hyphen join and its parts,
    where it had lost both and `IL6` missed the title; pairs run across
    Han, kana and Hangul as Lucene's do (`新しい` is `新し` `しい`), so
    any two characters or more of a stretch find it; ICU4X is given the
    whole stretch, Han and kana together, and each word it finds is
    divided again until it is a word alone; and each switch changes one
    thing — pairs no longer split `IL6` beside Han, which is the joins
    switch's to do. The tokens leave in the order of their positions, a
    form once at a position, which the engine requires. Properties hold
    each of these on generated text.
  - **T1.2 An index with it.** The mapping takes the switches; an opened
    index registers the analyzer its manifest names, so a query is split
    as its text was (I2); `search build` over the documents the epoch's
    index was built from — the merged and normalized sets, assembled as
    for the published index — with given switches into a directory of
    its own, never published; `evaluate run --index` against it. Built
    2026-10-03: `search build --out <dir>
    [--analyzer <name>]` (the name as reports print it, read back letter
    for letter; `--out` refused under the store's `search/` or where
    anything exists; without an analyzer the published mapping is built
    there, the control a treatment is measured against), the switches in
    the index's manifest — a field the
    published index's manifest does not carry, so it is written as it
    was and the search version does not move; an index opened only as it
    was built (text fields split by another analyzer than the manifest
    names are refused); a query split by the analyzer's query side;
    `evaluate run --index <dir>` and `search query --index <dir>`, the
    run's works still named and dated by the epoch, an index at another
    batch than the epoch's refused, the sidecar and the run's tag saying
    which index answered. Held by properties through a real index: every
    switch off answers as the published index does, score for score; and
    under any switches a word of a title finds its document.
  - **T1.3 The screening pieces (evaluation plan §14, E6).** The
    screening set — every work a suite judged and a seeded random sample
    of the rest — drawn once and kept; each working half's seeded query
    sample; each build's switches in its manifest. Built 2026-10-03, each
    piece drawn once and kept (its `--out` refused when it exists), with
    an `about.json` of what it was drawn from:
    - `evaluate sample <suite> --out <suite> --limit N`: N queries of
      the working half as a suite of its own, kept whole, none of the
      test half in it.
    - `evaluate screening-set <suite>... --out <dir> --limit N`: every
      work the suites judged, at any grade and in either half, or named
      as a seed, that the epoch's index holds, and N of the other works
      it holds; public ids, in `works.tsv`.
    - Both draws keep the items whose seeded hash is smallest, as the
      language key's does: a larger sample holds every smaller one, so a
      set can grow without being redrawn, and nothing depends on the
      order the items came in.
    - `search documents --works <set> --out <file>`: the set's works'
      documents, assembled once from the merged and normalized sets as
      the published index's are, and saved — a zstd stream of JSON
      lines. A build that assembled its own documents would read the
      whole corpus for every variant, which is the cost a screening
      index is for avoiding.
    - `search build --out <dir> --documents <file> [--analyzer <name>]`:
      an index of those documents and no others, with no assembly, its
      manifest naming the file and the set by their digests, so a run's
      sidecar names its screening set.
    On the sample store (110,469 works; a set of 5,033): the documents
    saved in 3.1 s, 2.8 MiB; an index built from them in under a second,
    against 4 s when each build assembled. How many works the set should
    sample is not chosen here (§8 item 8).
  - **T1.4 A check before any screen.** The all-off build of the
    screening set, run, must score as the published mapping's build of
    the same documents does — the control every treatment is measured
    against, since the epoch's own index holds other works and so scores
    otherwise; the machinery changes nothing by itself. Not to the last
    digit: two builds of one index differ in what the build's segments
    decide (§7 item 15), so the check is no difference but those. Built
    2026-10-03: `evaluate same <run> <run>`, which counts scores apart
    within 1e-5 of the score, ranks holding another of tied works, and
    works tied with the last a run returned, apart from real differences,
    and fails on any of those; two runs that asked other queries, or to
    another depth, are refused. Tried on the sample store, 2026-10-03.
    Twelve queries to depth 1,000 on every work: the all-off build
    against the epoch's index, 0 real differences in 10,792 lines, 116
    scores apart in their last digits and 22 places holding another of
    tied works; one all-off build against a second, the same kinds (112,
    24); every switch on, 20,388 real differences. Thirty queries on a
    screening set of 5,033 works: the all-off build against the control,
    0 real differences in 12,090 lines; every switch on, 19,431. The
    box's run is the check itself, and is to come.
  - **T1.5 D13 and D8 in the index.** GlotLID on every title and
    abstract at build time, answers cached by text hash on the claim; the
    English stemmed fields (title, abstract) with KStem, ported from
    Lucene, and perhaps current Porter2; the lexicon table (D10) as its
    own switch; the query searching the stemmed fields at a lower weight,
    a number tuned on the working halves alone (§14.6).
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
12. **Some of the key's labels are wrong.** Read in the texts a detector
    called English against their label (the score's samples, 2026-09-30):
    original-language titles tagged Russian or Italian that are English
    sentences, and other-language abstracts whose language attribute says
    Hungarian, Lithuanian, Spanish, Swedish or Turkish over English text.
    *Costs* the shares of other languages called English, which count
    those as the detector's errors. *Lifted by* reading the samples and a
    set of texts a person labels.
13. **ICU4X's dictionary model splits Thai, Lao, Khmer and Burmese**, not
    its LSTM model, ICU4X's default for them. The LSTM splits a word by
    its neighbours: in a sentence written for the analyzer's tests,
    `เบาหวาน` (diabetes) is one word in the title and `เบา` `หวาน` typed
    alone, so the query misses the title (I2); the dictionary model split
    every word of seven such sentences, one or two per script, as it
    splits the word alone. A dictionary's longest match can still depend
    on context — generated text found it, a mark of one script on a
    letter of another — so each word found is divided again until it is a
    word alone (2026-10-03): one word of a text, typed alone, is split as
    the text was, by construction. What is left: several words typed
    together, without the text around them, can be divided at other
    places than in the text; and those sentences are not the corpus's
    titles. *Costs* the LSTM's reading of words its dictionary lacks,
    which the dictionary splits wrongly (D2); words finer than ICU4X
    found them, where one divides alone; and I2 for phrases unmeasured
    on real titles. *Lifted by* T2's census (D11): every two and three
    adjacent words the analyzer finds in the corpus's titles in these
    scripts, and in Chinese and Japanese under `Words`, split again
    without the title, the share that splits otherwise counted; the LSTM
    model, divided the same way, is then a treatment to try against the
    dictionary.
14. **A word's other spellings count toward its field's length.** The
    index side writes `il6` beside `il` and `alpha` beside `α`, and
    Tantivy counts every term written as one more of the field's (its
    `num_tokens`, read in 0.26.1's `postings_writer.rs`), where Lucene
    leaves out terms that share a position. A title with hyphens, Greek
    letters or letters beside digits is scored as a longer title, each of
    its terms a little lower. *Costs* a second effect inside the joins
    and Greek switches' measured effect, which T2's screen cannot tell
    from the spellings meeting; none with every switch off. *Lifted by*
    the engine counting positions, not terms — a change to Tantivy or
    the next engine — or, short of that, by measuring it: the same
    switch with the extra spellings in a field of their own.
15. **Two builds of one index do not answer alike to the last digit.**
    Tantivy's writer divides the documents among segments as its threads
    take them; a document's score is its terms' scores added in an order
    the segment decides, and works tied on a score come in the order they
    lie in the index. Built twice from the same documents, an index gives
    scores apart in the seventh digit, and another of the tied works at a
    rank or at the depth (the sample store, 2026-10-03: 112 scores and 24
    ranks in 10,792 lines). *Costs* a run's exact reproduction — the same
    build must be kept, not rebuilt — and a little noise in every
    comparison of two builds, in ranks among tied works. *Lifted by* a
    build of one segment written by one thread, at the price of the
    build's time, or by the next engine.
16. **Under pairs, one character typed alone finds nothing.** `肺癌基因`
    is the pairs `肺癌` `癌基` `基因`; `癌` (cancer) typed alone is the
    term `癌`, which the index never wrote — nor `型` after a digit, in
    `2型糖尿病` (type 2 diabetes), where `2型` asks for `2` and `型`. As
    Lucene's CJK analyzer does by default. I1 says every word is
    findable, and Chinese has words of one character. *Costs* those
    words, alone or beside a digit or a Latin word, in Chinese, Japanese
    and Korean text under pairs; not under ICU4X's words, where the
    dictionary has them. *Lifted by* the index writing each character
    beside its pairs (Lucene's `outputUnigrams`), the query still asking
    pairs of anything longer — the user's call (§8 item 7); it adds a
    term a character to those texts, and item 14's cost with it.

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
6. **Labelling Crossref's titles, deferred.** A person reading a few
   hundred Crossref titles for their language would measure GlotLID
   outside PubMed, say how often a title and its abstract differ (whether
   borrowing is ever worth it), and ready T3's stemmers. Deferred
   2026-09-30: it wants readers of the languages — the user: "i don't
   think i'd personally be able to tell the difference between indic
   languages, or even reliably be able to distinguish japanese from
   chinese writing". Not needed for English stemming (D13, decided).
   Before T3.
7. **Single characters under pairs** (§7 item 16), the user's to decide:
   whether the index writes each Han, kana and Hangul character beside
   its pairs, so that `癌` and the `型` of `2型` are found. *Recommended:
   yes, as part of pairs and not a switch of its own* — I1 is an
   invariant, no judged set in those languages would measure the
   switch, and the index's size and build time with it are measured on
   the screening build either way. Before T2's screen.
8. **How large the screening pieces are**, not chosen: how many of the
   corpus's other works the screening set samples, and how many queries
   a suite's sample holds. Neither is guessed here. What can be
   measured on the box before any screen, since a larger draw holds
   every smaller one: the set at several sizes, each one's documents
   saved and its control built and run, for its build time, its index's
   size, and how far its control's figures on the sampled queries stand
   from the epoch's own on the same queries. *Recommended: measure at
   about 1, 4 and 16 million sampled works (roughly 0.5%, 2% and 8% of
   epoch 7's 188.7 million) with a quarter of RELISH's working half, the
   evaluation plan's own example (§14.2), and take the smallest set
   whose figures stand close to the next one's* — points to measure at,
   not estimates of what is enough. The user's to set. Before T2's
   screen.

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
- **2026-09-30, after D13's measurement (§3.3).** The user proposed
  assuming English unless a detector says otherwise, asked whether 86%
  should alarm, and then: "*are* we confident that pubmed is labelling
  english correctly? … what *is* that pubmed label actually labeling? …
  for convenience, authors will submit an english translation of their
  title, abstract while the real work is in their original language",
  and "we *should* add abstract scanning to titles that are too short. the
  language we care about for the purposes of indexing is the actual
  language of the text we're ingesting". D13 amended so (record tags never
  decide); the default-English and borrowing rules measured on a key that
  carries each title's abstract ("go ahead with the new key and extended
  score").
- **2026-09-30, after §3.4.** The user: "go with the recommendation" —
  D13 decided: GlotLID, English unless another language at 0.5 or above
  (0.5 against 0.7 in T2's screen), no borrowing for now, answers cached
  by text hash.
