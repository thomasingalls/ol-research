# Language identification: a survey (2026-09-29)

For the tokenizer plan's D13 (`docs/tokenizer-plan.md`): which detector
decides the language of each title and abstract when the lexical index
is built. The user's provisional pick is lingua-rs, "pending the outcome
of a survey of what's out there today, particularly in rust, but also
what exactly fastText derivatives are up to these days/if their crown has
been taken by another method". Criteria, in order: accuracy on short
academic titles; languages covered (every language matters); licence of
code and model; speed and memory on the box; pure Rust against a C++
build dependency.

This is a research agent's report from docs, READMEs, crate metadata,
papers and model cards, read 2026-09-29; nothing was installed or run.
Its figures are the sources' own, most of them self-reported, and not yet
checked here. The times for 250 million texts are straight division of
published rates, not measurements.

## Comparison

| Candidate | Kind | Languages | Short-text accuracy (source) | Speed & memory (source) | Licence code / model | Maintenance |
|---|---|---|---|---|---|---|
| **lingua-rs 1.8.0** | Rust-native. Naive Bayes over 1–5-grams plus a script rule engine | 75 | Tested on its own Leipzig test data, mean over 75 languages. High-accuracy mode: single words 74%, word pairs 89%, sentences 96%. Low-accuracy mode: 64 / 82 / 94 ([tables](https://github.com/pemistahl/lingua-rs/tree/main/tables)) | 2,000 sentences, all languages, high mode: 3.12 s on 1 thread, 0.41 s multi-threaded on an 8-core i9 ([README §5](https://github.com/pemistahl/lingua-rs); the README does not say which version produced this). heliport's benchmark: 100k sentences in 56.3 s on 1 thread ([heliport](https://github.com/ZJaume/heliport)). Memory is "a few dozen megabytes" with all languages since 1.8.0 (FST models); before that "at least hundreds of megabytes". Model files about 300 MB on disk ([v1.8.0 notes](https://github.com/pemistahl/lingua-rs/releases/tag/v1.8.0)) | Apache-2.0 / Apache-2.0 ([crates.io](https://crates.io/crates/lingua)) | 1.8.0 on 2026-03-09; commits through 2026-09 |
| langram 0.12.0 | Rust-native rewrite of lingua's approach; models memory-mapped | 188 models, plus 158 scripts with no model | Author's comparison on OpenLID text, the agent's means over their CSV (74 languages): words 83.6% vs lingua 70.8%; two words 94.0 vs 82.4; text 98.8 vs 95.6 ([compare repo](https://github.com/RoDmitry/lang_detectors_compare)). Trained on OpenLID ([langram_models](https://github.com/RoDmitry/langram_models)), so test and training data may overlap | 74 languages × 2,000 lines: 6.5 s vs lingua-high 103.6 s on an Intel 9700K; no model size published | MIT or Apache-2.0 / same (see licences) | 0.12.0 on 2026-05-11; about 5k downloads; one developer |
| whatlang 0.18.0 | Rust-native trigrams | 70 | lingua's tables: words 57%, pairs 72%, sentences 95%. Texts of 20 characters or fewer, 16 languages: 78.7% ([whichlang README](https://github.com/quickwit-oss/whichlang)) | 16.6 µs per short string (same source); no memory figure | MIT | 0.18.0 on 2025-10-16; used by Meilisearch's tokenizer `charabia` 0.10 |
| whichlang 0.1.1 | Rust-native logistic regression on hashed n-grams | 16 | Texts of 20 characters or fewer: 92.1% (README) | 0.26 µs per short string; no confidence returned ([src](https://github.com/quickwit-oss/whichlang)) | MIT | 0.1.1 on 2025-01-21; built for Quickwit |
| heliport | Rust port of HeLI-OTS (word and character n-grams) | 220 | No length-bucketed figures | 100k OpenLID sentences on 1 thread: 2.33 s, vs 8.44 s fastText-OpenLID and 1.12 s CLD2 ([README](https://github.com/ZJaume/heliport)) | GPL-3.0 | Last push 2026-09-21; not on crates.io |
| CLD2 via `cld2` | C++ binding | 83 ([CLD2](https://github.com/CLD2Owners/cld2)) | "not designed to do well on very short text" (built for 200+ characters); lingua's tables: words 34%, pairs 68% | 43,735 samples/s ([CommonLID T8](https://arxiv.org/abs/2601.18026)) | Unlicense wrapper / Apache-2.0 | Wrapper last released 2017, marked "DEPRECATED in favor of whatlang" ([rust-cld2](https://github.com/emk/rust-cld2)) |
| CLD3 via `cld3` | C++ binding (cxx) | about 100 | CommonLID, 76 shared languages: F1 61.6 | 2,026 samples/s (CommonLID) | Apache-2.0 | Crate last released 2023; upstream [archived](https://github.com/google/cld3) |
| fastText readers | [`fasttext` 0.8.0](https://github.com/messense/fasttext-rs) (now pure Rust, training and inference); [`fasttext-pure-rs` 0.1.0](https://github.com/silvermine/fasttext-pure-rs) (inference only, parity tests against C++ on lid.176.ftz for 35 languages); `cfasttext-sys` 0.7.8 (C++ via `cc`) | – | – | No published Rust figures | MIT | 2026-04-18 and 2026-04-08; upstream fastText [archived](https://github.com/facebookresearch/fastText) |
| lid.176 | fastText model | 176 | FLORES-200 F1 .796 vs GlotLID .987 on 108 shared languages; UDHR .647 vs .890 ([GlotLID paper T3](https://arxiv.org/abs/2310.16248)). Tatoeba 98.9% (trained on Tatoeba); OpenSubtitles 80.2% ([modelpredict, 2021](https://modelpredict.com/language-identification-survey)) | .bin 112k sentences/s, 136 MB; .ftz 93k/s, 3.5 MB (modelpredict) | MIT / **CC BY-SA 3.0** ([fasttext.cc](https://fasttext.cc/docs/en/language-identification.html)) | 2017 |
| **GlotLID v3** | fastText model | 2,102 labels (1,880 ISO codes); `und`/`zxx` labels for "no language" | README: "avoid using it on very short sentences" ([GlotLID](https://github.com/cisnlp/GlotLID)). FLORES+ F1 98.34, UDHR 97.39 ([OpenLID-v3 paper](https://arxiv.org/abs/2602.13139)). CommonLID, 76 core languages: F1 91.6, the best | 3,127 samples/s on an M4 Pro, threads not stated (CommonLID T8); model.bin 1.69 GB ([HF](https://huggingface.co/cis-lmu/glotlid)) | Apache-2.0 / Apache 2.0 plus notices on training data | v3 dated 2024-04 (CommonLID's text says "v4", its table v3; unresolved) |
| OpenLID-v2 / v3 | fastText models | 200 / 194 plus a noise class | FLORES+ F1 98.09 / 97.72; UDHR 96.27 / 96.14 (OpenLID-v3 paper) | v2 13,123 samples/s (CommonLID); files about 1.2 GB ([v2](https://huggingface.co/laurievb/OpenLID-v2), [v3](https://huggingface.co/HPLT/OpenLID-v3)) | Model **GPL-3.0** | v3 released 2026-02 by HPLT |
| NLLB lid218e | fastText model | 217 | CommonLID core languages: F1 77.7 | 10,867 samples/s; 1.18 GB | Model **CC BY-NC 4.0** ([HF](https://huggingface.co/facebook/fasttext-language-identification)) | 2023 |
| CommonLingua | Byte-level conv1D plus attention, PyTorch | 334 | CommonLID macro-F1 .788 vs GlotLID v3 .673 (self-reported); "not assessed on very short text segments" ([card](https://huggingface.co/PleIAs/CommonLingua)) | 553 texts/s on 8 CPU threads (bf16), 114/s on 1; 9.4 MB of weights | Apache-2.0 | Released 2026-04-28 |

Others found, with no independent evaluation: [langidentify
0.1.0](https://github.com/jlpka/langidentify) (84 languages, Wikipedia,
Apache-2.0, one release, benchmarked only against Java Lingua on 10
European languages); ports of older methods ([langid-rs
1.1.0](https://crates.io/crates/langid-rs), [langdetect-rs
0.2.3](https://crates.io/crates/langdetect-rs), py3langid_rs).

## Is fastText still the standard?

For large-scale identification, yes. FineWeb-2 uses GlotLID v3, leaving
lid.176 behind for coverage and separate scripts, and calls transformer
identifiers "too slow and expensive to run at a large scale"
([paper](https://arxiv.org/abs/2506.20920)); the same paper says FineWeb
v1, CC-100 and CulturaX used lid.176 and HPLT2 used OpenLID. HPLT 3.0
uses OpenLID-v2 ([paper](https://arxiv.org/abs/2511.01066)). Common
Corpus used "the fastText language identification model" ([ICLR
2026](https://arxiv.org/abs/2506.01732)). MADLAD-400 is the exception: an
in-house semi-supervised Transformer-Base model on 498 languages
([paper](https://arxiv.org/abs/2309.04662)), not found released.

The independent benchmark is CommonLID (ACL 2026, [arXiv
2601.18026](https://arxiv.org/abs/2601.18026)): human-labelled web lines,
109 languages, mean line 215.5 characters. Scores are lower than on
earlier benchmarks; "no clear top-performing model"; CLD2 and GlotLID sit
on the speed–accuracy frontier. Zero-shot on down-sampled data, GPT-5
scored F1 91.8 against GlotLID's 93.5 on core languages, 66.6 against
90.6 on African languages.

Nothing has taken the crown. LLMs lose to GlotLID at far higher cost;
transformers are too slow (AfroLID 66 samples/s). CommonLingua (byte-level,
2.35 M parameters) is the one credible challenger: +11.5 macro-F1 over
GlotLID on CommonLID paragraphs, self-reported, untested on short text,
hundreds of texts a second on CPU. [UniLID](https://arxiv.org/abs/2602.17655)
(ICML 2026) and [ConLID](https://arxiv.org/abs/2506.15304) report being
competitive or a little better; on
[DIVERS-Bench](https://arxiv.org/abs/2509.17768) ConLID and GlotLID
average 77.3 and 77.8 F1.

Short text is unsolved: "short sentences are much more likely to be
mislabeled, across all models and datasets" (OpenLID-v3 authors); GlotLID,
CLD2 and [FUN-LangID](https://github.com/google-research/url-nlp/tree/main/fun_langid)
all warn against very short inputs.

## Short text and scholarly text

Benchmarks: FLORES-200/FLORES+ (single sentences, Wikipedia);
[UDHR-LID](https://huggingface.co/datasets/cis-lmu/udhr-lid) (418
languages, 9–2,120 characters); [WiLI-2018](https://arxiv.org/abs/1801.07779)
(235 languages, Wikipedia paragraphs); Tatoeba (short, but in lid.176's
training data); CommonLID (drops lines under 10 characters); TweetLID via
DIVERS-Bench. Only lingua and its derivatives report accuracy at one and
two words, all self-reported; no independent head-to-head of lingua
against GlotLID or OpenLID on short inputs was found.

No benchmark on academic titles was found. OpenAlex runs langdetect (55
languages) on the abstract, or the title when there is none; a JASIST
2025 study found it overstates English: 75% English in the metadata, 68%
on manual check ([arXiv 2409.10633](https://arxiv.org/abs/2409.10633)).
AllenAI's peS2o runs pycld3 on titles and abstracts separately
([card](https://huggingface.co/datasets/allenai/peS2o)).

## Licences (a reading, not legal advice)

Clean for commercial use: lingua (Apache-2.0), GlotLID (Apache-2.0, with a
notice on training data), whatlang, whichlang, langram, CommonLingua —
though langram declares MIT/Apache for models trained on OpenLID's
sources, described only as allowing "at minimum free use for research",
unresolved. lid.176 is CC BY-SA 3.0 (attribution; share-alike on
adaptation). OpenLID v1–v3 and heliport are GPL-3.0: copyleft applies on
distribution, so running one inside our own build is likely fine and
shipping it in a binary would not be. NLLB lid218e is non-commercial: out.

## Speed and memory on the box

No Zen 3 figures exist for any candidate. Straight division of published
rates for 250 million texts (titles are shorter than the benchmarks'
sentences): lingua high mode ≈4,900 sentences/s on an 8-core i9, ≈14 h;
GlotLID 3,127/s on an M4 Pro (threads unstated), ≈22 h; OpenLID-v2
13,123/s, ≈5 h; CommonLingua 553/s on 8 threads, ≈5 days. Memory fits the
box for all: lingua tens of MB, the fastText models 1.2–1.7 GB, lid.176.ftz
under 1 MB. Pure Rust: lingua, langram, whatlang, whichlang, heliport;
fastText models through `fasttext` 0.8 or `fasttext-pure-rs`, but neither
is shown to load GlotLID or OpenLID `.bin` files (only lid.176.ftz parity
is documented); the C++ route needs a C++ toolchain in the builder.

## The agent's recommendation

1. Short titles: lingua's design and its self-reported numbers favour it
   at one to three words; the fastText models warn against very short
   text; at 5–20 words the published numbers are close (lingua 96–99% on
   its data, GlotLID and OpenLID about 98% F1 on FLORES+). The evidence
   does not separate them; it has to be measured.
2. Languages: lingua's weak point — 75 languages, none of Galician,
   Uzbek, Kyrgyz, Tajik, Nepali, Sinhala, Malayalam, Kannada, Odia,
   Burmese, Khmer, Lao, Amharic, Pashto, Kurdish, Uyghur, Tibetan,
   Maltese, Luxembourgish; its probabilities sum to one over its own
   languages, so a Galician title comes out as one of them (Portuguese or
   Spanish, likely). What it returns for a script none of its languages
   use: not verified. GlotLID has 2,102 labels and explicit `und`/`zxx`.
3. Licence: lingua and GlotLID both clean.
4. Speed and memory: both feasible in one build at hours, not days;
   lingua lighter on memory.
5. Pure Rust: lingua is; GlotLID plausible through the new pure-Rust
   fastText crates, unproven.

Verdict: lingua-rs is defensible for the English-first decision on short
titles but cannot meet "every language"; the best alternative is GlotLID
v3; neither dominates on published evidence, so measure both (and
langram, cheap to try, weakly evaluated). A design to test: GlotLID sets
the label and abstains through `und`/`zxx`, lingua confirms within its 75
languages, a text is stemmed only when both agree above a threshold.

What to measure on PubMed before choosing: English titles of English
articles, NLM's original-language titles, abstracts with a language
attribute (strip the brackets from NLM's translated titles; they are
translations, and labels carry some noise); English precision and recall,
and per language where samples allow, by length (1–3, 4–8, 9–20, over 20
words, abstracts), with bootstrap intervals; calibration (confidence
against correctness, a threshold per detector for a target English
precision); out-of-set behaviour on title-length slices of FLORES+ and
UDHR-LID in languages lingua lacks; scholarly hazards (Latin binomials,
gene and chemical names, acronyms, all-capital titles, mixed titles,
transliterations, Han-only Japanese titles); throughput and peak memory at
16 threads on the box, and load time. PubMed is biomedical and weighted
toward European languages, so other sources for the rest.

## Not verified

Whether the Rust fastText crates load GlotLID or OpenLID `.bin` files;
which lingua version produced the README speed figures; the hardware and
lingua version behind heliport's table; CommonLID's thread counts and its
GlotLID "v4"/"v3" labelling; whether MADLAD's model is released; the
licence terms of lingua's Leipzig training corpora; any benchmark on
academic titles (none found).
