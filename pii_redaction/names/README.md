# Name lists

`given_names.txt` and `surnames.txt` are taken from [Wikidata](https://www.wikidata.org)
(data licensed CC0), downloaded on 2026-10-01 via the [QLever](https://qlever.dev) SPARQL
endpoint. They contain the labels (in the languages mul, de, en, fr, es, it, nl, pt, tr,
pl) of all items that are an instance of:

- given names: given name (Q202444), male given name (Q12308941), female given name
  (Q11879590), unisex given name (Q3409032)
- surnames: family name (Q101352), Wikidata item for family name (Q121493679)

Only single capitalized words of at least 2 letters are kept (hyphenated names such as
`Anne-Sophie` are included). Query used for the given names (for surnames, replace the
classes in `VALUES`):

```sparql
PREFIX wd: <http://www.wikidata.org/entity/>
PREFIX wdt: <http://www.wikidata.org/prop/direct/>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
SELECT DISTINCT ?s WHERE {
  VALUES ?c { wd:Q202444 wd:Q12308941 wd:Q11879590 wd:Q3409032 }
  ?x wdt:P31 ?c . ?x rdfs:label ?l .
  FILTER(LANG(?l) IN ("mul","de","en","fr","es","it","nl","pt","tr","pl"))
  BIND(STR(?l) AS ?s)
}
```

`stop_words.txt` is maintained by hand: words in the lists that are usually not names.
