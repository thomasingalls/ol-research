@default:
    just --list

# metadata downloads for proof of concept parser development
metadata-download-2026-09-14-0001:
	aria2c --file-allocation=none --seed-time=0 -d data/raw/crossref https://academictorrents.com/download/f692802461d1277d14551526ecb292a9637af254.torrent
	curl -fSL --create-dirs -o data/raw/pubmed/pubmed26n1334.xml.gz https://ftp.ncbi.nlm.nih.gov/pubmed/baseline/pubmed26n1334.xml.gz
	curl -fSL --create-dirs -o data/raw/openalex/updated_date=2025-07-25/part_0000.gz https://openalex.s3.amazonaws.com/data/jsonl/works/updated_date=2025-07-25/part_0000.gz
