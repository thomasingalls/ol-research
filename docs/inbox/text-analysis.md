# Text analysis for scientific search

**Status:** notes, 2026-09-24 — a conversation about the user's thesis on
analysis (tokenization, stemming, lemmatization, stop words) for BM25 over
scientific text, recorded at the user's request. Speculative, not a plan;
the indexing policy is the user's to set, and the mapping in
`crates/search/src/schema.rs` stays provisional until it is. Companion to
the [retrieval frontier](retrieval-frontier.md) and [benchmarks for search
and research](retrieval-benchmarks.md). Every fact is marked **P**
(primary: Tantivy 0.26.1's source, or text run through its analyzers in a
scratch crate on 2026-09-24; for papers, the abstract), **S** (secondary
only) or **U** (not verified).

**Since, 2026-09-25:** the search box no longer goes through
`QueryParser`. `crates/query-parser` reads what was typed into a tree,
and `search::translate` builds the `TermQuery`, `PhraseQuery` and
`BooleanQuery` by hand, splitting each word with the field's own analyzer
for now (serving plan §7). So the query side that §2 and idea 6 describe
as hand-built is the one there is: analyzing a query differently from the
index is a change to the translation, not to a parser. The tokenizer as a
project of its own is serving plan §11, item 11; the query language's
second pass, item 36; the order proposed for what comes next, item 37.
The plan for the tokenizer, draft 1 of 2026-09-29, is
[tokenizer plan](../tokenizer-plan.md): its first finding, that Chinese,
Japanese and Thai text yields no terms at all, is its §3.

## 1. The thesis

In the user's words:

1. Stemming, lemmatization, tokenization and stop words are a leading
   cause of bad BM25 retrieval scores in scientific search — term
   confusion and the like.
2. It is a finite and totally solvable problem.

## 2. How Tantivy does analysis, read from Solr

Tantivy's model is Lucene's with fewer parts and no schema.xml (all **P**):

| Solr | Tantivy 0.26 |
|---|---|
| `fieldType` with an `<analyzer>` chain | A `TextAnalyzer`: one tokenizer, then token filters in order, registered by name on the index; a field names it with `TextFieldIndexing::set_tokenizer`. |
| The chain lives in the schema | Only the **name** is in the index's metadata. A custom analyzer is registered in code every time the index is opened, writer and reader both, or indexing and parsing fail. |
| Index and query analyzers apart | None. `QueryParser` runs the field's analyzer over the query. A different query-time analysis means building `TermQuery`, `PhraseQuery` and `BooleanQuery` by hand. |
| `copyField` | None; the same text is added to two fields by the caller. |
| Char filters, WordDelimiterGraph, SynonymGraph, KStem, Hunspell, protected words | **None.** On hand: lowercasing, ASCII folding, Snowball stemmers, stop-word lists, a long-token filter, an alphanumeric-only filter, dictionary decompounding; simple, whitespace, raw, regex and n-gram tokenizers. |
| An analysis plugin | Implement `Tokenizer` or `TokenFilter` in Rust (small traits), or analyze anywhere and hand in `PreTokenizedString`: finished tokens with offsets and positions. |

The analyzers registered by default are `raw`, `whitespace`, `default` and
`en_stem`. `default` splits on every character that is not alphanumeric
(Unicode's reading, so Greek letters are kept), drops tokens over 40
bytes and lowercases; `en_stem` adds Snowball's English stemmer (Porter2)
after lowercasing.

On the query and scoring side (all **P**):

- BM25's `k1 = 1.2` and `b = 0.75` are constants in the scorer, not
  settings.
- A query over several fields sums a BM25 score per field; it is not
  BM25F.
- The parser's default operator is OR.
- A query word that analyzes to several tokens becomes a phrase query,
  which needs positions indexed: `IL-6` is searched as the phrase `il 6`.

## 3. What the current mapping does to scientific text

`crates/search/src/schema.rs` indexes title, abstract, authors and venue
as `TEXT`, which is `default`: no stemming, no stop words. Markup is gone
before the text arrives (`crates/merge/src/document.rs` strips titles;
the source views strip abstracts), so `CO<sub>2</sub>` reaches the index
as `CO2`.

`default` over some scientific strings (**P**, run):

| Text | Tokens | Consequence |
|---|---|---|
| `IL-6` · `IL6` · `interleukin 6` | `il 6` · `il6` · `interleukin 6` | Three spellings of one thing; the first and last share only `6` |
| `TNF-α` · `TNF-alpha` | `tnf α` · `tnf alpha` | No match; ASCII folding leaves `α` alone too |
| `β-lactamase` | `β lactamase` | Same again for `beta-lactamase` |
| `SARS-CoV-2` · `COVID-19` | `sars cov 2` · `covid 19` | Only as phrases; `2` and `19` alone are near-useless terms |
| `C++` · `C#` · `.NET` | `c` · `c` · `net` | Two languages become a letter |
| `Na+/K+-ATPase` | `na k atpase` | Charges and the pump's name lost |
| `2,3,7,8-tetrachlorodibenzo-p-dioxin` | `2 3 7 8 tetrachlorodibenzo p dioxin` | Locants become loose digits |
| `ALL` · `CAT` | `all` · `cat` | The leukemia and the gene meet the words |
| `T-cell` · `E. coli` · `k-means` | `t cell` · `e coli` · `k means` | Single letters, rescued only by phrases |
| `p < 0.05` · `5 mg/kg` | `p 0 05` · `5 mg kg` | Numbers become noise terms |
| `tumour` · `tumor` | two terms | British and American apart |
| `δ13C` | `δ13c` | One token here, three for `δ 13C` |
| `Müller` · `Mueller` · `Muller` | three terms | Folding joins the first and last, never `Mueller` |
| `O'Brien` · `Jean-Pierre` | `o brien` · `jean pierre` | The authors field shares the chain |

What `en_stem` would add (**P**, run):

| Kind | Examples |
|---|---|
| Joined wrongly | `experiment`, `experience` → `experi`; `organ`, `organism`, `organization` → `organ`; `university`, `universe` → `univers`; `operating`, `operation`, `operative` → `oper` |
| Left apart wrongly | `analysis` → `analysi` but `analyses` → `analys`; `mouse` → `mous`, `mice` → `mice`; `tumour`, `tumor`; `absorb`, `absorption` → `absorpt` |
| Joined rightly | `modeling`, `modelling` → `model`; `genes` → `gene`; `species`, `specie` → `speci` |

And Tantivy's built-in English stop list (**P**, run): `vitamin A` →
`vitamin`, `hepatitis A` → `hepatitis`, `WAS protein` → `protein`, `IT
infrastructure` → `infrastructure`, `AS patients` → `patients`, `OR 1.5`
→ `1 5`. It keeps `all`, `us` and `i`.

## 4. What the literature says

- **Harman 1991**, "How effective is suffixing?" (JASIS 42(1), **P**,
  abstract): three suffixing algorithms on Cranfield, Medlars and CACM;
  no significant improvement for any. A failure analysis suggested
  weighting term variants differently and stemming selectively by query
  length and term importance.
- **Hull 1996**, "Stemming algorithms: a case study for detailed
  evaluation" (JASIS 47(1), **S**, through surveys): on TREC, stemming
  almost always helps, but modestly, and no significant difference
  between suffix strippers like Porter and Lovins.
- **Jiang and Zhai 2007**, "An empirical study of tokenization strategies
  for biomedical information retrieval" (Information Retrieval 10,
  341–363, [doi](https://doi.org/10.1007/s10791-007-9027-7), **P**,
  abstract): on all the TREC biomedical collections, tokenization changed
  accuracy by up to 80 %; different query types want different
  heuristics; stemming helps only some queries; stop-word removal in
  general does not help.

Read together: in general English the stemmer barely matters on average;
in biomedical text tokenization matters a great deal, and per query more
than on average. That supports thesis (1) for scientific text. It is not
measured on this corpus, and the corpus has causes of its own to
separate from it: works with a title only scored against works with an
abstract, a sum per field instead of BM25F, OR by default.

## 5. Finite, and how far solvable

**Finite, mostly yes.** The vocabulary at any snapshot is a list: Tantivy
hands over its term dictionary with document frequencies. It is
Zipfian, so a head of modest size carries nearly all the mass queries
touch. And the *kinds* of confusion are few: hyphen, space or joined;
Greek symbol or name; acronyms only case tells apart; British or
American; irregular inflection; stemmer collisions; names carrying
punctuation; digits glued to letters.

**Totally, no — three limits.**

1. **The kinds are finite; the instances are not.** Vocabulary grows
   with the corpus without end (Heaps' law) and new gene, model and virus
   names arrive daily. Solved means rules per kind that generalize, and
   a reviewed table for the head — not a closed list.
2. **Some confusions are context, not form.** `ALL`, `AS`, `IT`, `WAS`;
   `cell` in biology and in telecoms; `transformer`. A function of one
   token cannot be right both ways. The ambiguous forms can be listed;
   they cannot be resolved by analysis alone.
3. **The largest BM25 failure is concept mismatch.** `heart attack`,
   `myocardial infarction`, `MI`: synonymy, not morphology. Large but
   finite in biomedicine (MeSH entry terms, UMLS); open outside it. And
   the work that never names the concept at all is not finite: that is
   what dense or learned retrieval buys.

So: finite at the layer the thesis names, large-finite one layer up,
unbounded above that.

## 6. Ideas

1. **Measure the ceiling first.** On a benchmark with judgments (§1 of
   the [benchmarks](retrieval-benchmarks.md): TREC-COVID, NFCorpus,
   SciFact, BioASQ), take every relevant work BM25 missed or ranked low
   and say why: (a) the query's term is there in another form —
   analysis would fix it; (b) a synonym or abbreviation — a thesaurus
   would; (c) the concept is never named; (d) something else — no
   abstract, field weighting. A model can label, with spot checks. The
   share in (a) is the direct test of thesis (1), and says whether the
   rest is worth weeks. The sample cannot run this meaningfully; the full
   PubMed baseline can.
2. **Analysis as a reviewed table, not an algorithm.** Instead of Porter,
   a lookup from surface form to index term or terms, built from the
   index's own term dictionary: candidate groups by the kinds in §5,
   the head reviewed, the tail left to rules, unknown forms through the
   conservative default. Versioned like the merge, with a report of what
   each version changed; the index manifest's `mapping` field
   (`crates/search/src/index.rs`) is where the version would be named.
   In Tantivy it is one custom `TokenFilter`. It makes "finite" literal:
   what ships is a file that can be diffed.
3. **Variant pairs from the merge.** One work id often carries the title
   from PubMed, Crossref and OpenAlex: texts known to mean the same
   thing, differing in exactly the ways §3 lists. Aligned word by word
   they give variant pairs with ground truth, from data already held.
   Titles as cited in references may give more (not checked how much of
   that text is kept).
4. **Layered fields, not one chain.** Title and abstract indexed two or
   three ways — near-exact, normalized, stemmed — all queried, the exact
   weighted highest. A collision then cannot outrank an exact hit, so
   over-stemming stops costing. The price is index size on a SATA disk
   shared with the search engine; measurable.
5. **Read case before lowercasing discards it.** A filter ahead of
   `LowerCaser` that protects all-caps tokens of two to five letters
   (`ALL`, `CAT`) and single letters after `vitamin`, `hepatitis`,
   `type`. And no stop list: IDF already discounts `the`, and §3 shows
   what the list does to `vitamin A`.
6. **Query time for context, index time for form.** Orthographic
   variants are true equivalents and belong in the index. Synonyms and
   abbreviations depend on context and belong in the query, built by
   hand rather than through `QueryParser` (§2), so they can change
   without a rebuild.
7. **Biomedicine has a head start.** NLM's SPECIALIST Lexicon and its
   Lexical Tools (LVG) exist for exactly this — inflections, spelling
   variants, normalization — and UMLS's normalized strings are built with
   them. The Lexical Tools FAQ calls the package "a free open source
   project for public users" allowing "use and redistribution without
   warranty", under NLM's terms (**P**,
   [FAQ](https://lhncbc.nlm.nih.gov/LSG/Projects/lvg/current/web/faq.html));
   the Lexicon is distributed as a UMLS Knowledge Source and as open
   source subject to terms (**S**,
   [UMLS manual](https://www.ncbi.nlm.nih.gov/books/NBK9680/)). Whether
   the Lexicon needs a UMLS license is **U**: read the terms before
   depending on it.

## 7. Open questions

1. What share of BM25's misses on this corpus analysis could fix (§6.1).
2. Whether the disk allows layered fields beside the search engine.
3. What the authors and venue fields want: names want folding and
   transliteration (`Mueller`), not stemming; venues want abbreviations
   (`J Biol Chem`).
4. Which kinds go at index time and which at query time (§6.6), kind by
   kind.
5. Whether the SPECIALIST terms fit the project's (§6.7).
