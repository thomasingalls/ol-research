# Knowing when a classification is wrong

The question, the user's, 2026-09-19: *do we have a mechanism to discover
that our classifications are mistaken?*

It came out of the Elsevier registrant list, where the answer happened to
be yes. This note is about whether that generalises. It does not.

**The short answer.** We have good detectors exactly where somebody built
one by hand, and none anywhere else. There is no general machinery. Every
counter that could tell us a rule is wrong was written deliberately for
that rule, and the rules nobody wrote a counter for are silent.

This note inventories what exists, names the gaps, and proposes the one
pattern worth repeating. It is an assessment of the codebase as it stood
on 2026-09-19; line numbers will move, so the types and fields are named
rather than located.

## 1. `--survey` is not this instrument

Worth separating first, because the name misleads.

At normalize time, `--survey` (`ingest::run::Survey`) collects records
that **failed to normalize** — every distinct failure signature with a
count and three examples — and exits non-zero. At merge time it does two
things: `identity::Stats::unmapped` counts a source vocabulary value the
tables do not have instead of stopping, and `Identity::resolve(survey)`
counts a conflict component the §3.3 rules cannot resolve, by `hairball`
or `unmodelled`, with examples.

All three are *this input broke something*. None of them fires on a value
that parsed cleanly and was filed in the wrong box. `--survey` is a
crash-batching tool, and it is a good one; it is not a correctness tool.

## 2. The one detector that actually worked

`PublisherIds::off_list`, in the normalize stats sink. It asks:

> **How often does my rule's precondition fail?**

Concretely: how often does something shaped exactly like an Elsevier PII
sit under a registrant the list says is not Elsevier. It prints on every
`normalize` run whether or not `--survey` is passed.

On 2026-09-19 it did its whole job. It named 53 off-list registrants
carrying 2,587 PII-shaped values, and two of them — 10.1006 (Academic
Press) and 10.1074 (ASBMB) — turned out to be misfiled. They were added
to `merge::publisher::ELSEVIER` on that evidence.

Three properties made it work, and all three are worth copying:

1. It counts the cases **just outside** the rule, not the cases inside it.
2. It prints unconditionally, so nobody has to remember a flag.
3. It names the *dimension* of the near-miss — the registrant — so the
   count is actionable rather than merely alarming.

It is the only counter of its kind in the codebase.

Note where it lives: the **normalize** stats sink, over raw files. The
merge never runs it.

## 3. What else exists

In rough order of strength.

**Cross-source field disagreement.** `merge::build::FieldStat` carries
`present` / `compared` / `disputed` plus a per-source breakdown, for
title, authors, venue, published, language, **form**, **genre**, access
and abstract, printed by `report::merge` as a rate per field and per
source pair. This means a mis-mapped vocabulary entry — PubMed's
publication-type table, Crossref's work types — does have a feedback
path: its dispute rate against the other two sources rises. **It is the
only classification in the codebase with a standing, quantitative
wrongness signal.**

**Two of our own tables contradicting each other.** `kind_conflicts`:
works whose promoted genre cannot live in its promoted form
(`Form::admits`), reported with the offending pair flagged.

**A rule catching itself.** `identity::Stats::collided` — publisher ids
one source claimed for more than one of its own records, and therefore
not identifiers at all. A classification discovering its own error, with
values written out.

**Confidence, recorded.** `components.uncorroborated` (works joined on an
untestable title), and the `Labels` shape strings — `two dois`, `doi
spellings`, `two pmids`, `wrong link`, `shared name`, `resolved without a
rule` — counted with eight examples each. Every conflict component says
which rule resolved it, so a shape we do not believe is visible by
reading. This is the closest thing to an audit that exists.

**Resolution disagreement.** `record::Why::Disagree` — a reference whose
names resolve to different works — plus the reference and relation target
tables (held / stub / self / no name / shared / disagree / empty /
unknown), and the edge agreement matrix over edges where both sources
hold the citing work.

**Within-source disagreement at normalize.** `PubmedStats::doi_disagree`
(a record's two DOI slots holding different valid DOIs) and
`CrossrefStats::owner_differs`.

**Churn over time.** The registry's `minted / reused / merged / split /
attached / dropped`. A classification change that moves identity shows up
as splits and merges between runs. **The only longitudinal signal we
have** — everything else above is within-run.

**Invariants.** `graph::check::invariants` and `serve::compare::
differences`. Both verify that two computations of the same rules agree.
They cannot tell us the rules are wrong, only that they were applied
consistently. (And `serve::compare` has a blind spot of its own: it does
not walk the `cited_as` set. See the record-index note.)

## 4. The gaps

Classifications that nothing in the codebase would ever contradict.

1. **Every `publisher::Refusal`, at merge time.** Both sources' assert
   paths do `if let Ok(name) = scope.name(value)` — `source-pubmed/src/
   lib.rs:126`, `source-crossref/src/lib.rs:76` — and drop the refusal on
   the floor. The merge reports never say how many candidate ids were
   thrown away or why. The only counter is in a different command over
   raw files that nobody has to run before a merge. `ArticleNumber` alone
   discards 31,506 rows on the rule that a bare number never names a
   work. That rule is probably right; nothing anywhere would tell us if
   some publisher's genuine id were a bare number.

2. **The Elsevier rewrite, once a registrant is on the list.** Adding
   10.1006 makes its counter go quiet *by construction*. Nothing then
   watches for the failure mode the rewrite introduces — two different
   articles ending up with one canonical publisher id under one
   registrant. It was checked by hand on 2026-09-19 and is zero. There is
   no standing counter, and `collided` does not cover it, because
   `collided` only sees one source claiming a value for two of its own
   records.

3. **The title thresholds.** `Agreement::Similar` — one edit per ten
   characters on a long title — counts as agreement, as do `Prefix` and
   `Vernacular`. Nothing records how often the `Similar` verdict rather
   than `Equal` was what let two records merge. Loosen that constant and
   works merge that should not; no number moves.

4. **Single-source classifications.** `disputed` can only fire where two
   sources both hold the work *and* both supply the field. A PubMed-only
   record's genre has no second opinion, and `place()`'s
   qualifier-versus-genre decision — which publication types say nothing
   about genre — has no counter at all, in any pass.

5. **Status flags.** Retracted / corrected / concern are reported by
   *which* sources assert them, but with no dispute rate. A flag one
   source asserts is indistinguishable from a flag only one source knows
   about.

6. **The PubMed exclusion list.** Counted by reason on the way out;
   nothing ever reconsiders. An excluded record is not there to disagree
   with anything.

7. **Stubs.** 5.08M of them, each carrying the classification *this name
   names a work nobody holds*. Nothing counts a stub that is really a
   held work under another id.

8. **Search.** The provisional Tantivy mapping has no relevance
   measurement whatsoever, and `serve::compare` only proves two cores are
   equally wrong. Serving item 11 (indexing policy) is where this lands.

## 5. The shape of it

Every wrongness detector in this codebase is one of three kinds:

- a rule's **precondition failing** (the Elsevier counter — one instance);
- **two sources disagreeing** (the fields table — the real instrument);
- **two of our own tables contradicting each other** (form versus genre).

Which leaves one blind spot that no amount of more of the same would
close:

> **Nothing detects a classification that is uniformly, confidently wrong
> across all sources.** If PubMed and Crossref both call something a
> review, and our table maps *review* to the wrong genre, every number
> stays green.

That case needs an outside reference — a labelled sample, a spot check
against another system, or a human reading — and we have none of those
wired in.

## 6. The pattern worth repeating

For each hand-built table or threshold, a counter for the cases that sit
**just outside** it, printed unconditionally, named by the dimension of
the near-miss.

Candidates, in the order their absence seems to cost most:

1. Refusals surfaced in the **merge** report, per `Refusal` kind, with
   examples — closing gap 1, and making gap 2 visible for free if the
   canonical-collision check rides along.
2. `Agreement::Similar`-as-decider counted separately from `Equal`,
   closing gap 3 at the cost of one counter.
3. A near-miss counter on `place()`'s qualifier decision, closing the
   half of gap 4 that is ours rather than the data's.

None of these is a plan; they are the shape a plan would take. Each is
one counter and no rule change.

## 7. The ambition this sits under, not yet actionable

The user's, 2026-09-19:

> we do, in particular, want a dashboard of counts, summary stats, etc to
> keep track of the shape of the collection, metadata delivery frequency,
> processing time, etc over time, but this is ill defined right now, so
> not exactly actionable.

Recorded as an ambition, deliberately unspecified. Two observations that
bear on it whenever it is taken up:

- **Almost everything above is within-run.** Reports print and are gone.
  The registry's churn line is the only number the store keeps across
  runs. A dashboard over time is a different artifact from any report we
  write today, and the first question it raises is where the series is
  stored — not what is on it.
- **The counters a dashboard would want are largely the ones that already
  exist.** The gap between "we print this" and "we can plot this" is
  retention and identity of the series, not new measurement. The
  exception is §6's near-miss counters, which do not exist yet and which
  are exactly the series that would show a rule going wrong before
  anybody notices downstream.

See also: `docs/inbox/metadata-quality.md` (what bad metadata actually
looks like, measured), `docs/inbox/record-index.md` (the `serve::compare`
blind spot), and serving plan item 11 (indexing policy).
