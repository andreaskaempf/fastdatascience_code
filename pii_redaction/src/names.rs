// Detection of first and last names (and titles such as "Herr" or "Dr."),
// replacing them with numbered tokens as in the target data, e.g.
// "Anna Maria Schmidt" -> "[GIVENNAME1] [GIVENNAME2] [LASTNAME1]".
//
// Names cannot be recognized by their shape, so three kinds of rules are used,
// in this order:
// 1. Labelled fields, e.g. "Vorname: Anna", "last_name": "Schmidt", <Title>Dr.</Title>
// 2. Context rules, e.g. "Herr Schmidt", "Mein Name ist Anna", "Kommentar von Anna Schmidt"
// 3. Name lists: a whole delimited value made up of names, e.g. "Royer Pasquot"
//    in JSON, or a given name followed by another name, e.g. "Anna Schmidt"
// The name lists come from Wikidata, see names/README.md; the words in
// names/stop_words.txt are never taken as names.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::{Captures, Regex};

// Name lists and stop words, built into the binary. Ambiguous names are also
// ordinary words (e.g. Haus, Plan, Very); they only count as names next to an
// unambiguous name or in a strong context such as "Herr" or "Nachname:".
static GIVEN_NAMES: LazyLock<HashSet<&str>> =
    LazyLock::new(|| include_str!("../names/given_names.txt").lines().collect());
static SURNAMES: LazyLock<HashSet<&str>> =
    LazyLock::new(|| include_str!("../names/surnames.txt").lines().collect());
static AMBIGUOUS_NAMES: LazyLock<HashSet<&str>> =
    LazyLock::new(|| include_str!("../names/ambiguous_names.txt").lines().collect());
static STOP_WORDS: LazyLock<HashSet<&str>> = LazyLock::new(|| {
    include_str!("../names/stop_words.txt")
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
});

// Building blocks for the patterns: a capitalized name word (possibly
// hyphenated, e.g. Anne-Sophie, O'Brien), a lowercase particle such as "von",
// and a sequence of name words separated by single spaces
const WORD: &str = r"\p{Lu}[\p{Ll}'’]+(?:-\p{Lu}[\p{Ll}'’]+)*";
// German "der"/"den" are only particles after von/van, otherwise they are articles
const PARTICLE: &str =
    r"(?:von der|von den|van der|van den|de la|von|van|de|del|della|di|da|du|la|le|ten|ter|zu|al|el|bin|ibn)";
// Titles after which any capitalized word is taken as a name, e.g. "Herr Reineke"
const CORE_TITLE: &str = r"(?:Herrn|Herr|Frau|Fräulein|Frl\.?|Dr\.|Prof\.|Doktorin|Doktor|Mrs\.?|Mr\.?|Ms\.?|Miss|Mx\.?|Sir|Dame|Madame|Monsieur|Mme\.?|Mlle\.?)";
// Titles that are also ordinary words or abbreviations, after which only a word
// in a name list is taken, e.g. "Bruder Anselm", "Stadträtin Kohlweg", "Prz Louis"
const EXTENDED_TITLE: &str = r"(?:Gräfin|Graf|Grf\.?|Baronin|Baron|Bar\.?|Freiherr|Freifrau|Fürstin|Fürst|Prinzessin|Prinz|Prz\.?|Herzogin|Herzog|Hrz\.?|Erzherzogin|Erzherzog|Ehz\.?|Königin|König|Kön\.?|Kaiserin|Kaiser|Erbprinzessin|Erbprinz|Erbin|Erb\.?|Pfarrerin|Pfarrer|Pastorin|Pastor|Pater|Bruder|Br\.?|Schwester|Schw\.?|Mutter|Vater|Äbtissin|Abt|Prälat|Papst|Kardinal|Bischof|Reverend|Rev\.?|Rabbi|Imam|Stadträtin|Stadtrat|Bürgermeisterin|Bürgermeister|Ministerin|Minister|Senatorin|Senator|Kommandeur|Kapitän|General|Oberst|Major|Rechtsanwältin|Rechtsanwalt|Anwältin|Anwalt|Rätin|Rat)";

fn name_sequence(max_extra_words: usize) -> String {
    format!(r"{WORD}(?: (?:{PARTICLE} )*{WORD}){{0,{max_extra_words}}}")
}

// A single word of a particle; "der"/"den" only occur in matches after von/van
fn is_particle(word: &str) -> bool {
    static PARTICLES: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(&format!("^(?:{PARTICLE}|der|den)$")).unwrap());
    PARTICLES.is_match(word)
}

// Checks a word against a name list; hyphenated words also match if all parts do
fn in_list(list: &HashSet<&str>, word: &str) -> bool {
    !STOP_WORDS.contains(word)
        && (list.contains(word) || (word.contains('-') && word.split('-').all(|p| list.contains(p))))
}

fn is_given(word: &str) -> bool {
    in_list(&GIVEN_NAMES, word)
}

fn is_surname(word: &str) -> bool {
    in_list(&SURNAMES, word)
}

fn is_name(word: &str) -> bool {
    is_given(word) || is_surname(word)
}

// A name that is not also an ordinary word
fn is_unambiguous(word: &str) -> bool {
    is_name(word) && !AMBIGUOUS_NAMES.contains(word) && !word.split('-').any(|p| AMBIGUOUS_NAMES.contains(p))
}

// Requirement for the first word of a name
#[derive(Clone, Copy, PartialEq)]
enum First {
    Any,         // any capitalized word that is not a stop word
    InList,      // a word in a name list
    Unambiguous, // a word in a name list that is not also an ordinary word
}

fn first_ok(word: &str, first: First) -> bool {
    !STOP_WORDS.contains(word)
        && match first {
            First::Any => true,
            First::InList => is_name(word),
            First::Unambiguous => is_unambiguous(word),
        }
}

// What a sequence of name words is known to be
#[derive(Clone, Copy)]
enum Kind {
    Given,        // all words are given names, e.g. after "Vorname:"
    Last,         // all words are last names, e.g. after "Nachname:"
    Full(Single), // first word(s) given names, then last names; how to treat a single word
}

// How to label a single word of a full name, if the name lists do not decide
#[derive(Clone, Copy)]
enum Single {
    Given,
    Last,
}

// Replaces the name words in a space-separated sequence with numbered tokens,
// keeping particles, e.g. "Anna von Berg" -> "[GIVENNAME1] von [LASTNAME1]".
// `start` is the number of the first token, e.g. 2 after "Zweiter Vorname:".
fn label_words(words: &[&str], kind: Kind, start: usize) -> String {
    let name_count = words.iter().filter(|w| !is_particle(w)).count();
    let (mut given, mut last) = match kind {
        Kind::Given => (start, 1),
        Kind::Last => (1, start),
        Kind::Full(_) => (1, 1),
    };
    let mut seen_last = false;
    let mut index = 0;
    let mut labelled = Vec::new();
    for word in words {
        if is_particle(word) {
            labelled.push(word.to_string());
            continue;
        }
        let as_given = match kind {
            Kind::Given => true,
            Kind::Last => false,
            Kind::Full(single) if name_count == 1 => match (is_given(word), is_surname(word)) {
                (true, false) => true,
                (false, true) => false,
                _ => matches!(single, Single::Given),
            },
            // First word is a given name, further words only while no last name
            // has been seen and they are given names but not surnames
            Kind::Full(_) => index == 0 || (!seen_last && is_given(word) && !is_surname(word)),
        };
        index += 1;
        if as_given {
            labelled.push(format!("[GIVENNAME{given}]"));
            given += 1;
        } else {
            labelled.push(format!("[LASTNAME{last}]"));
            last += 1;
            seen_last = true;
        }
    }
    labelled.join(" ")
}

// Number of leading words of a sequence to take as a name: the first word (if
// it meets the requirement), then further words while they are in a name list.
// Particles are only taken in between.
fn name_prefix_len(words: &[&str], first: First) -> usize {
    if !first_ok(words[0], first) {
        return 0;
    }
    let mut taken = 1;
    let mut i = 1;
    while i < words.len() {
        let mut j = i;
        while j < words.len() && is_particle(words[j]) {
            j += 1;
        }
        if j < words.len() && is_name(words[j]) {
            taken = j + 1;
            i = j + 1;
        } else {
            break;
        }
    }
    taken
}

// Labels the leading name words of a matched sequence and keeps the rest unchanged
fn label_prefix(sequence: &str, kind: Kind, first: First) -> Option<String> {
    let words: Vec<&str> = sequence.split(' ').collect();
    let n = name_prefix_len(&words, first);
    if n == 0 {
        return None;
    }
    let mut result = label_words(&words[..n], kind, 1);
    for word in &words[n..] {
        result.push(' ');
        result.push_str(word);
    }
    Some(result)
}

// Start number for a labelled field, from a digit in the label ("Vorname 2",
// "lastname1") or a word like "zweiter" or "second"
fn label_number(label: &str) -> usize {
    let lower = label.to_lowercase();
    if let Some(d) = lower.chars().find(|c| c.is_ascii_digit()) {
        return d.to_digit(10).unwrap() as usize;
    }
    if lower.contains("zweit") || lower.contains("second") || lower.contains("middle") {
        2
    } else if lower.contains("dritt") || lower.contains("third") {
        3
    } else {
        1
    }
}

// Separator between a label and its value, e.g. ": ", "\": \"", ">", ":** "
const SEPARATOR: &str = r#"[ \t*"']*[:=>][ \t*"']*"#;

// 1. Labelled fields. The label is kept, the value replaced.
static TITLE_FIELD: LazyLock<Regex> = LazyLock::new(|| {
    // e.g. "Titel: Dr.", <Title>Fräulein</Title>, "- Anrede: Herr"
    Regex::new(&format!(
        r"(?P<label>\b(?i:titel|title|anrede|salutation){SEPARATOR})(?P<value>\p{{Lu}}[\p{{L}}-]*\.?(?: \p{{Lu}}[\p{{L}}-]*\.?){{0,2}})"
    ))
    .unwrap()
});
static GIVEN_FIELD: LazyLock<Regex> = LazyLock::new(|| {
    // e.g. "Vorname: Anna", "Zweiter Vorname: Maria", "givenname1": "Anna", <FirstName>
    Regex::new(&format!(
        r"(?P<label>\b(?i:(?:zweiter |dritter )?vorname[n]?|rufname|first[_ ]?name|given[_ ]?name|givename|second[_ ]?name|middle[_ ]?name)(?:[ _]?[1-9])?{SEPARATOR})(?P<value>{})",
        name_sequence(3)
    ))
    .unwrap()
});
static LAST_FIELD: LazyLock<Regex> = LazyLock::new(|| {
    // e.g. "Nachname: Schmidt", "Zweiter Nachname: Berg", "last_name": "Schmidt"
    Regex::new(&format!(
        r"(?P<label>\b(?i:(?:zweiter |dritter )?nachname|familienname|geburtsname|surname|last[_ ]?name|family[_ ]?name)(?:[ _]?[1-9])?{SEPARATOR})(?P<value>{})",
        name_sequence(3)
    ))
    .unwrap()
});
static FULL_FIELD: LazyLock<Regex> = LazyLock::new(|| {
    // e.g. "Name: Anna Schmidt", "Teilnehmer: Schmidt"; at least one word must
    // be an unambiguous name (see replace_field)
    Regex::new(&format!(
        r"(?P<label>\b(?i:name|full[_ ]?name|vollständiger name|patient(?:in)?|teilnehmer(?:in)?|mitarbeiter(?:in)?|kunde|kundin|ansprechpartner(?:in)?|kontaktperson|verantwortliche[r]?)(?:[ _]?[1-9])?{SEPARATOR})(?P<value>{})",
        name_sequence(3)
    ))
    .unwrap()
});

// 2. Context rules
static TITLE_CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    // One or more titles followed by a name, e.g. "Herr Schmidt", "Frau Dr. Anna Berg",
    // "Bruder Anselm"
    Regex::new(&format!(
        r"\b(?P<titles>(?:(?:{CORE_TITLE}|{EXTENDED_TITLE}) )+)(?P<name>{})",
        name_sequence(2)
    ))
    .unwrap()
});
static NAME_PHRASE: LazyLock<Regex> = LazyLock::new(|| {
    // Phrases that introduce a name; the name need not be in a list, e.g. "Mein Name ist Anna"
    Regex::new(&format!(
        r"(?P<pre>\b(?i:mein name ist|mein vorname ist|mein nachname ist|ich heiße|ich heisse|my name is|my first name is|my last name is) )(?P<name>{})",
        name_sequence(3)
    ))
    .unwrap()
});
static AUTHOR_PHRASE: LazyLock<Regex> = LazyLock::new(|| {
    // Phrases after which a name is likely, e.g. "Kommentar von Anna Schmidt",
    // "erstellt von Schmidt", "written by Anna"
    Regex::new(&format!(
        r"(?P<pre>\b(?:(?i:kommentar|artikel|beitrag|feedback|nachricht|bericht|brief|anfrage|antrag|foto|bild|zitat|unterstützung|hilfe) von|(?i:geschrieben|erstellt|verfasst|unterzeichnet|unterschrieben|gesendet|genehmigt|geprüft) von|(?i:written|created|posted|sent|signed|approved) by) )(?P<name>{})",
        name_sequence(3)
    ))
    .unwrap()
});
static GREETING: LazyLock<Regex> = LazyLock::new(|| {
    // Greetings followed by a name, e.g. "Liebe Anna,", "Hallo Peter", "Sehr geehrte Anna Schmidt"
    Regex::new(&format!(
        r"(?P<pre>\b(?:Liebe|Lieber|Liebes|Hallo|Hi|Hey|Dear|Servus|Moin|Sehr geehrte[rs]?(?:/r)?|Guten Tag|Guten Morgen)[ ,]+)(?P<name>{})",
        name_sequence(3)
    ))
    .unwrap()
});
static SIGNATURE: LazyLock<Regex> = LazyLock::new(|| {
    // Name on the line after a closing, or after "gez.", e.g.
    // "Mit freundlichen Grüßen\nAnna Schmidt"; also with literal "\n" as in escaped JSON
    Regex::new(&format!(
        r"(?P<pre>(?:(?i:mit freundlichen grüßen|freundliche grüße|viele grüße|beste grüße|liebe grüße|herzliche grüße|schöne grüße|best regards|kind regards|regards|sincerely|cheers)[,.!]?[ \t]*(?:(?:\r?\n|\\n)[ \t]*)+)|gez\.[ \t]+)(?P<name>{})",
        name_sequence(3)
    ))
    .unwrap()
});

// 3. Name list rules
static DELIMITED_VALUE: LazyLock<Regex> = LazyLock::new(|| {
    // A whole value that consists of name words, e.g. "Royer Pasquot" in JSON,
    // ", Martirano," in CSV, "| Costin |" in a table, "- Anna Schmidt" in a list,
    // <strong>Gerter</strong>, **Sabatella**. The end of the value is checked
    // separately (see VALUE_END), so that adjacent values can share a delimiter.
    Regex::new(&format!(
        r#"(?m)(?P<pre>(?:^[ \t]*(?:[-*•]|\d+\.)?|["'>|;,\[{{(]|\\"|\*\*)[ \t]*)(?P<name>{})"#,
        name_sequence(3)
    ))
    .unwrap()
});
static VALUE_END: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^[ \t]*(?:["'<|;,\]})\r\n*]|\\"|\\n|$)"#).unwrap());
// A JSON or Python dict key rather than a value, e.g. "Lukas": [...]
static KEY_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^[ \t]*(?:["']|\\")[ \t]*:"#).unwrap());
static WORD_RUN: LazyLock<Regex> = LazyLock::new(|| {
    // Runs of up to 4 capitalized words, in which a given name followed by another
    // name word is taken as a full name, e.g. "... wie Anna Schmidt sagte"
    Regex::new(&format!(r"\b{}", name_sequence(3))).unwrap()
});

fn is_core_title(word: &str) -> bool {
    static CORE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!("^{CORE_TITLE}$")).unwrap());
    CORE.is_match(word)
}

fn is_title(word: &str) -> bool {
    static ANY: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(&format!("^(?:{CORE_TITLE}|{EXTENDED_TITLE})$")).unwrap());
    ANY.is_match(word)
}

// Applies a labelled-field rule. For full names, at least one word must be an
// unambiguous name, to leave e.g. "Name: Projekt Alpha" intact.
fn replace_field(text: &str, re: &Regex, kind: Kind, require_unambiguous: bool) -> String {
    re.replace_all(text, |caps: &Captures| {
        let words: Vec<&str> = caps["value"].split(' ').collect();
        if require_unambiguous && !words.iter().any(|w| is_unambiguous(w)) {
            return caps[0].to_string();
        }
        format!("{}{}", &caps["label"], label_words(&words, kind, label_number(&caps["label"])))
    })
    .into_owned()
}

// Applies a context rule: text before the name ("pre") is kept
fn replace_context(text: &str, re: &Regex, kind: Kind, first: First) -> String {
    re.replace_all(text, |caps: &Captures| match label_prefix(&caps["name"], kind, first) {
        Some(name) => format!("{}{}", &caps["pre"], name),
        None => caps[0].to_string(),
    })
    .into_owned()
}

// Take a string and replace names and titles with tokens
pub fn redact_names(text: &str) -> String {
    // 1a. Labelled title fields, e.g. "Titel: Dr.". A lowercase <title> is the
    // title of an HTML document, so there only known titles are replaced.
    let mut result = TITLE_FIELD
        .replace_all(text, |caps: &Captures| {
            let words: Vec<&str> = caps["value"].split(' ').collect();
            if caps["label"].starts_with("title>") && !words.iter().all(|w| is_title(w)) {
                return caps[0].to_string();
            }
            let titles = words.iter().map(|_| "[TITLE]").collect::<Vec<_>>().join(" ");
            format!("{}{}", &caps["label"], titles)
        })
        .into_owned();

    // 2a. Titles followed by a name; before the other labelled fields, so that
    // "Name: Herr Schmidt" is not taken as first name "Herr". After a title, a
    // single word is usually a last name ("Herr Schmidt"); the titles are replaced
    // too. After core titles any word is taken, after others only names in a list.
    result = TITLE_CONTEXT
        .replace_all(&result, |caps: &Captures| {
            let first = if caps["titles"].split_whitespace().any(is_core_title) { First::Any } else { First::InList };
            match label_prefix(&caps["name"], Kind::Full(Single::Last), first) {
                Some(name) => {
                    let titles = caps["titles"].split_whitespace().map(|_| "[TITLE] ").collect::<String>();
                    format!("{titles}{name}")
                }
                None => caps[0].to_string(),
            }
        })
        .into_owned();

    // 1b. Other labelled fields
    result = replace_field(&result, &GIVEN_FIELD, Kind::Given, false);
    result = replace_field(&result, &LAST_FIELD, Kind::Last, false);
    result = replace_field(&result, &FULL_FIELD, Kind::Full(Single::Given), true);

    // 2b. Other context rules
    result = NAME_PHRASE
        .replace_all(&result, |caps: &Captures| {
            let pre = caps["pre"].to_lowercase();
            let kind = if pre.contains("vorname") || pre.contains("first") {
                Kind::Given
            } else if pre.contains("nachname") || pre.contains("last") {
                Kind::Last
            } else {
                Kind::Full(Single::Given)
            };
            match label_prefix(&caps["name"], kind, First::Any) {
                Some(name) => format!("{}{}", &caps["pre"], name),
                None => caps[0].to_string(),
            }
        })
        .into_owned();
    result = replace_context(&result, &AUTHOR_PHRASE, Kind::Full(Single::Given), First::Unambiguous);
    result = replace_context(&result, &GREETING, Kind::Full(Single::Given), First::Unambiguous);
    result = replace_context(&result, &SIGNATURE, Kind::Full(Single::Given), First::Unambiguous);

    // 3a. Delimited values made up of name words only, at least one of them
    // unambiguous (a single word must be unambiguous), or of titles only,
    // e.g. ", Vater," or <em>Dame</em>
    let text = result;
    result = DELIMITED_VALUE
        .replace_all(&text, |caps: &Captures| {
            let end = caps.get(0).unwrap().end();
            if !VALUE_END.is_match(&text[end..]) || KEY_END.is_match(&text[end..]) {
                return caps[0].to_string();
            }
            let words: Vec<&str> = caps["name"].split(' ').collect();
            if words.iter().all(|w| is_title(w)) {
                return format!("{}{}", &caps["pre"], words.iter().map(|_| "[TITLE]").collect::<Vec<_>>().join(" "));
            }
            let names: Vec<&str> = words.iter().filter(|w| !is_particle(w)).copied().collect();
            if !STOP_WORDS.contains(names[0])
                && (names.len() > 1 || names[0].chars().count() >= 3)
                && names.iter().all(|w| is_name(w))
                && names.iter().any(|w| is_unambiguous(w))
            {
                format!("{}{}", &caps["pre"], label_words(&words, Kind::Full(Single::Given), 1))
            } else {
                caps[0].to_string()
            }
        })
        .into_owned();

    // 3b. Within each run of capitalized words: a given name followed by at
    // least one more name word, with at least one of them unambiguous
    WORD_RUN
        .replace_all(&result, |caps: &Captures| {
            let words: Vec<&str> = caps[0].split(' ').collect();
            let mut out: Vec<String> = Vec::new();
            let mut i = 0;
            while i < words.len() {
                if is_given(words[i]) {
                    let n = name_prefix_len(&words[i..], First::InList);
                    let names: Vec<&str> = words[i..i + n].iter().filter(|w| !is_particle(w)).copied().collect();
                    if names.len() >= 2 && names.iter().any(|w| is_unambiguous(w)) {
                        out.push(label_words(&words[i..i + n], Kind::Full(Single::Given), 1));
                        i += n;
                        continue;
                    }
                }
                out.push(words[i].to_string());
                i += 1;
            }
            out.join(" ")
        })
        .into_owned()
}
