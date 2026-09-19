A. Finishing the batch
1339 still stops — a stub minted for leftover names cited together with another stub (serving §11 item 25). Last of the five update files with an unresolved cause.
The record index (merge item 22 / serving item 18). This is the one I'd reach for. It retires in_text, held_beside, by_text — every derived mark that two merges must compute identically — and both of today's bugs were exactly that sync failing.
Item 26's blind spot. citers_of hard-codes held_beside: false; a reference landing on a held work may name another held work, which nothing records.
The three refusals are unmeasured beyond three hold-out files.
Batch speed (item 17) and paging across a fold (item 20).
B. Scale — things that only bite at corpus size
The corpus itself. Everything so far is a sample: 148,199 held works against 5,079,128 stubs. Full ingestion gates G3 in the graph plan and most of the questions below.
The registry at scale (merge item 10). It's JSON lines — today's merge appended 1,271,346 of them at ~94 bytes each. Binary log plus a memory-mapped fold.
Pass 2's memory: 5.0 GB resident on the sample (merge item 16).
Old artifact directories (serving item 15) — just became concrete: v1.2.3 and v2.7 are both sitting in the store now.
Replay after a compaction (item 16), the search engine at scale (item 4), the journal read whole on every poll (item 19).
C. Identity and data quality
Stubs at scale (merge item 4): 5.08M stubs from 5.87M names, 4.5M of them single-name. What they actually are matters.
Reference-level assertions (merge item 14) — are they true? 849 of them, and the merge found 338 are stubs of their own.
Lineage (merge item 1) — you said not now, but it's the real answer to the above, and today's splits were mostly version/erratum confusion.
The OpenAlex location rule — settled in discussion, never built.
Author and venue identity (item 6), Crossref alternative-id/is-same-as as name sources (item 9), OpenAlex records disagreeing with themselves (item 18), translations (12), journal-issue DOIs (17), the title test's blind spot (13).
D. Serving and product
Indexing policy (serving item 11) — explicitly yours, deliberately undecided; the provisional mapping is standing in, and it's the ceiling on search quality right now.
Feeds (item 2) — the first-seen measurement, then a feed registry. This is the acquisition half of freshness.
Old epochs and reader handoff (item 10), request logging and metrics (14), expand's budget (7), terms in code (3), the consistency alarm threshold (9).
E. Loose ends from today
Serving plan §7 still calls the JSON facade "later if wanted" and §8's command list has no --http. Both need a line.
page means page size in the JSON API — a rename to page_size is one word from you.
The frontend has no tests.

* plain-text citation parser
* plain-text citation -> doi matching engine
* proper search engine indexing hygiene (stemming, synonyms, etc)
* search ranking
* full text support for search
   * this requires a lot of thought, it's not a near term idea. 
* graph traversals in the api and in the ui
* pdf/html/image ingestion to get citation data off of an authoritative source. probably image -> small vision-language model -> deterministic citation parser -> our metadata. can be a genuine challenge, since some journals (looking at you nature, cell) have historically compressed citations to impossibly small signals. 
* supplemental data associated with an article
   * files from publisher
   * code/repo links
   * links to large datasets hosted elsewhere
   * ...
* some kind of measurement for search quality. maybe that's just spot checking top-20 results against google scholar / semantic scholar for a handful of queries, but i would prefer something more systematic.
* search ux
  * search w/in a results set
  * save an entire list of matching id's
  * advanced/faceted search
    * date, publisher, author, venue, ... maybe not faceting as extensive as pubmed's, but i want to eventually deliver equivalent precision at lower esoteric effort required
* accounts, probably
  * saved searches
* integrating academic family tree data
* multiple languages. english dominates published academic work, but it's not the only language works are published in
* typeahead search
* search suggestions
* search field highlighting
* make an *actual* exhaustive list of people who have made this product.
* allow csl-driven results listings.if you want your ui in your esoteric citation style, you can have that.
* export/save any article list
* mcp/skill
* probably special treatment for books, book chapters, series, journal/issue/special, ... may be worth building a browse view. 
    * it is *imperative* that we never make a special, divergent kind of work for other publication types. everything has to fit into the same schema. if it can't, we need to adjust the schema. we should look towards crossref, openalex, pubmed, and semanticscholar for inspiration as to how to treat this data.
