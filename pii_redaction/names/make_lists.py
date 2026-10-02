# Build the name lists used by src/names.rs from Wikidata (CC0), via the QLever
# SPARQL endpoint. Writes given_names.txt, surnames.txt and ambiguous_names.txt
# (names that are also German/English words or place names) into the directory
# of this script. Run: python3 make_lists.py

import os
import time
import urllib.error
import urllib.parse
import urllib.request

ENDPOINT = "https://qlever.dev/api/wikidata"
PREFIXES = """PREFIX wd: <http://www.wikidata.org/entity/>
PREFIX wdt: <http://www.wikidata.org/prop/direct/>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
PREFIX dct: <http://purl.org/dc/terms/>
PREFIX wikibase: <http://wikiba.se/ontology#>
PREFIX ontolex: <http://www.w3.org/ns/lemon/ontolex#>
"""
LANGS = '"mul","de","en","fr","es","it","nl","pt","tr","pl"'

# Labels of all items that are an instance of one of the given classes
NAMES_QUERY = PREFIXES + """SELECT DISTINCT ?s WHERE {{
  VALUES ?c {{ {classes} }} ?x wdt:P31 ?c . ?x rdfs:label ?l .
  FILTER(LANG(?l) IN ({langs})) BIND(STR(?l) AS ?s) }}"""
GIVEN_CLASSES = "wd:Q202444 wd:Q12308941 wd:Q11879590 wd:Q3409032"  # given name, male, female, unisex
SURNAME_CLASSES = "wd:Q101352 wd:Q121493679"  # family name, Wikidata item for family name

# Place names, which are often also surnames (e.g. Berlin, Deutschland): countries,
# sovereign states, states (federated, German, US), provinces, first-level
# administrative divisions, cities, big cities, capitals, German municipalities
PLACE_CLASSES = ("wd:Q6256 wd:Q3624078 wd:Q107390 wd:Q1221156 wd:Q35657 wd:Q34876 wd:Q10864048 "
                 "wd:Q515 wd:Q1549591 wd:Q5119 wd:Q262166")

# All word forms of German (Q188) and English (Q1860) lexemes, except proper nouns (Q147276)
WORDS_QUERY = PREFIXES + """SELECT DISTINCT ?w WHERE {
  VALUES ?lang { wd:Q188 wd:Q1860 } ?l dct:language ?lang ; wikibase:lexicalCategory ?cat ;
  ontolex:lexicalForm ?f . FILTER(?cat != wd:Q147276)
  ?f ontolex:representation ?rep . BIND(STR(?rep) AS ?w) }"""

# Frequent names that are also ordinary words, but should still count as names
KNOWN_NAMES = """
Müller Schneider Fischer Weber Meyer Wagner Becker Schulz Hoffmann Koch Bauer Richter
Klein Wolf Schröder Neumann Schwarz Zimmermann Braun Krüger Hofmann Hartmann Lange
Schmitt Werner Krause Meier Lehmann Schmid Schulze Maier Köhler Herrmann König Walter
Mayer Huber Kaiser Fuchs Peters Lang Scholz Möller Weiß Jung Hahn Schubert Vogel
Friedrich Keller Günther Frank Berger Winkler Roth Beck Lorenz Baumann Franke Albrecht
Schuster Simon Ludwig Böhm Winter Kraus Martin Schumacher Krämer Vogt Stein Jäger Otto
Sommer Groß Seidel Heinrich Brandt Haas Schreiber Graf Schulte Dietrich Ziegler Kuhn
Kühn Pohl Engel Horn Busch Bergmann Thomas Voigt Sauer Arnold Wolff Pfeiffer Smith
Baker Miller Cook Taylor Turner Walker Wright Hill Young King Green
Peter Frank Mark Paul Hans Klaus Karl Otto Ernst Fritz Max Rose Grace Will Bill Jack
Hope Eva Lina Mia Emma Lea Ben Tim Tom Jan Kai Leon Luca Noah Finn Elias Felix Jonas
""".split()


def query(q, attempts=3):
    data = urllib.parse.urlencode({"query": q}).encode()
    req = urllib.request.Request(ENDPOINT, data=data, headers={"Accept": "text/tab-separated-values"})
    for attempt in range(attempts):
        try:
            with urllib.request.urlopen(req, timeout=600) as resp:
                lines = resp.read().decode("utf-8").split("\n")[1:]  # skip header
            return {line.strip().strip('"') for line in lines if line.strip()}
        except urllib.error.HTTPError as e:  # the endpoint occasionally fails under load
            if attempt == attempts - 1:
                raise
            print(f"HTTP error {e.code}, retrying in 30 s")
            time.sleep(30)


def is_name_word(w):
    """Single capitalized word of 2+ letters, possibly hyphenated (Anne-Sophie),
    with an apostrophe (O'Brien) or an inner capital after Mc/Mac/De/... (McDonald)."""
    if len(w) < 2:
        return False
    for part in w.split("-"):
        if not part or not part[0].isupper():
            return False
        letters = part.replace("'", "").replace("’", "")
        if not letters.isalpha():
            return False
        if not letters[1:].islower() and not part.startswith(("Mc", "Mac", "De", "Di", "La", "Le", "O'", "D'")):
            return False
    return True


def write(name, words):
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), name)
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(sorted(words)) + "\n")
    print(f"{name}: {len(words)}")


given = {w for w in query(NAMES_QUERY.format(classes=GIVEN_CLASSES, langs=LANGS)) if is_name_word(w)}
surnames = {w for w in query(NAMES_QUERY.format(classes=SURNAME_CLASSES, langs=LANGS)) if is_name_word(w)}
words = query(WORDS_QUERY) | query(NAMES_QUERY.format(classes=PLACE_CLASSES, langs=LANGS))
ambiguous = {w for w in given | surnames if (w in words or w.lower() in words) and w not in KNOWN_NAMES}

write("given_names.txt", given)
write("surnames.txt", surnames)
write("ambiguous_names.txt", ambiguous)
