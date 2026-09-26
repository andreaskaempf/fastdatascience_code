// Read JSON file of German texts with PII, and redact the following:
// - IBAN bank numbers
// - Dates and times
// - Steuernummer tax IDs
// - IP addresses (IPv4 and IPv6)
// - Social security numbers
// - Phone numbers
// - Email addresses
// - Postal addresses
// - Driver's license numbers (Führerschein)
// - ID card numbers (Personalausweis)

use std::fs::File;
use std::io::{BufRead, BufReader, Result};
use std::sync::LazyLock;

use regex::Regex;

use serde::{Deserialize, Serialize}; // do NOT use the Result provided by Serde

fn main() -> Result<()> {
    // Read input file
    let file = File::open("data/texts.json")?;
    let reader = BufReader::new(file);

    // Process each line
    let mut n = 0;
    let mut matches = 0;
    for line in reader.lines() {
        let ok = process_line(line?.as_str())?;
        if ok {
            matches += 1;
        }
        n += 1;
    }
    println!("{} / {} match", matches, n);
    Ok(())
}

// Structure for one entry that comes in JSON
#[derive(Serialize, Deserialize, Debug)]
struct Entry {
    source: String,
    target: String,
}

// Process one entry, provided in JSON text, by showing original,
// redacted, and target text
fn process_line(text: &str) -> Result<bool> {
    // Parse the JSON into source and target text
    let data: Entry = serde_json::from_str(text)?;

    // Redact the text, check if matches target
    let redacted = redact(&data.source)?;
    let matches = redacted == data.target;

    // Display results if no match
    if !matches {
    println!("Source text:\n{}", data.source);
    println!("\nTarget text:\n{}\n", data.target);
    println!("\nRedacted text:\n{}\n", redacted);
    println!("- - - - -\n");
    }
    Ok(matches)
}

// Date patterns, compiled once on first use. Each pattern is surrounded by \b
// so that e.g. parts of IP addresses or longer digit strings are not matched.
// Note that these could be more strict by checking numeric values against expected ranges.
static DATE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"\b\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:?\d{2})?", // ISO timestamp e.g. 1943-06-04T00:00:00
        r"\b\d{1,2}/\d{1,2}/\d{4}\b",   // dd/mm/yyyy
        r"\b\d{1,2}\.\d{1,2}\.\d{4}\b", // dd.mm.yyyy
        r"\b\d{4}-\d{1,2}-\d{1,2}\b",   // yyyy-mm-dd (ISO 8601)
        r"\b\d{1,2}\.\d{1,2}\.\d{2}\b", // dd.mm.yy       e.g. 05.01.64
        r"\b\d{1,2}-\d{1,2}-\d{4}\b",   // dd-mm-yyyy     e.g. 05-01-2064
        r"\b\d{4}/\d{1,2}/\d{1,2}\b",   // yyyy/mm/dd     e.g. 2064/01/05
        r"\b\d{1,2}\.\s?(Januar|Februar|März|April|Mai|Juni|Juli|August|September|Oktober|November|Dezember)\s\d{4}\b",
                                        // dd. Monat yyyy e.g. 5. Januar 2064
        r"\b\d{1,2}\.\s?(Jan|Feb|Mär|Apr|Mai|Jun|Jul|Aug|Sep|Okt|Nov|Dez)\.?\s\d{4}\b",
                                        // dd. Mon yyyy   e.g. 5. Jan. 2064
        r"\b(Januar|Februar|März|April|Mai|Juni|Juli|August|September|Oktober|November|Dezember)\s\d{4}\b",
                                        // Monat yyyy     e.g. Januar 2064
        r"\b\d{1,2}\.\d{1,2}\.\B", // dd.mm. without year, e.g. "am 05.01." (\B: not followed by a digit or letter)
    ]
    .iter()
    .map(|p| Regex::new(p).expect("invalid date regex"))
    .collect()
});

// Time patterns, as (regex, replacement) pairs; the order matters, since
// "10.00 Uhr" must be handled before "00 Uhr". Hours and minutes are checked
// against valid ranges, and a following "Uhr" is kept in the output.
static TIME_PATTERNS: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    [
        // hh:mm(:ss), e.g. 14:30, 7:57, 12:36:41. Not directly after ":" or a
        // letter/digit, to leave IPv6 addresses such as fe80::1:23 intact.
        (r"(?P<pre>^|[^:0-9A-Za-z])(?:[01]?\d|2[0-3]):[0-5]\d(?::[0-5]\d)?\b", "${pre}[TIME]"),
        (r"\b(?:[01]?\d|2[0-4])\.[0-5]\d(?P<uhr>\s?Uhr)\b", "[TIME]${uhr}"), // hh.mm Uhr e.g. 10.00 Uhr
        (r"\b(?:[01]?\d|2[0-4])(?P<uhr>\s?Uhr)\b", "[TIME]${uhr}"), // hh Uhr     e.g. 9 Uhr
        (r"\b(?:[01]?\d|2[0-4])h\b", "[TIME]"),                     // hhh        e.g. 21h (may also be a duration)
        // Candidates for further formats:
        // (r"(?P<label>\b(?i:Uhrzeit|Zeitpunkt|Time)[\s:*\x22'>]{1,6})(?:[01]?\d|2[0-3])\b", "${label}[TIME]"),
        //                                                              // bare hour after a keyword, e.g. "Uhrzeit: 13"
        // (r"\b(?:[01]?\d|2[0-4])(?P<range>\s?[-–]\s?)(?:[01]?\d|2[0-4])(?P<uhr>\s?Uhr)\b", "[TIME]${range}[TIME]${uhr}"),
        //                                                              // range      e.g. 9-17 Uhr (otherwise "9-[TIME] Uhr")
        // (r"\b(?:[01]?\d|2[0-3])[.:]?[0-5]\d\s?h\b", "[TIME]"),        // hh:mm h    e.g. 14.30 h, 1430h
        // (r"\b(?:[01]?\d|2[0-3])h[0-5]\d\b", "[TIME]"),                // hhhmm      e.g. 14h30
        // (r"(?i)\b(?:1[0-2]|0?[1-9])(?::[0-5]\d)?\s?(?:a\.m\.|p\.m\.|(?:am|pm)\b)", "[TIME]"),
        //                                                              // am/pm      e.g. 2 pm; careful: German "am" = "on", as in "13:24 am 2021-07-11"
        // (r"(?i)\b(?:halb|viertel nach|viertel vor|dreiviertel)\s(?:eins|zwei|drei|vier|fünf|sechs|sieben|acht|neun|zehn|elf|zwölf)\b", "[TIME]"),
        //                                                              // spoken     e.g. halb drei
    ]
    .iter()
    .map(|(p, r)| (Regex::new(p).expect("invalid time regex"), *r))
    .collect()
});

// IBAN patterns: 2-letter country code, 2 check digits, then 11-30 alphanumeric
// characters (total length 15-34). The checksum is not verified, since sample
// texts often contain made-up IBANs such as DE12345678901234567890.
static IBAN_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"\b[A-Z]{2}\d{2}[A-Z0-9]{11,30}\b", // compact       e.g. DE89370400440532013000
        r"\b[A-Z]{2}\d{2}(?: [A-Z0-9]{4}){2,7}(?: [A-Z0-9]{1,3})?\b", // grouped e.g. DE89 3704 0044 0532 0130 00
        // Candidates for further formats:
        // r"\b[A-Z]{2}\d{2}(?:-[A-Z0-9]{4}){2,7}(?:-[A-Z0-9]{1,3})?\b", // grouped with dashes
        // r"(?i)\b[a-z]{2}\d{2}[a-z0-9]{11,30}\b", // lowercase, e.g. de89370400440532013000
    ]
    .iter()
    .map(|p| Regex::new(p).expect("invalid IBAN regex"))
    .collect()
});

// Steuernummer patterns, as (regex, replacement) pairs. Unformatted numbers are
// only matched after a keyword, because bare 10-13 digit numbers are often ID
// card or social security numbers. The keyword itself is kept in the output.
static TAX_ID_PATTERNS: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    [
        // State formats with slashes: FF/BBB/UUUUP (e.g. BW, Berlin, Hamburg),
        // FFF/BBB/UUUUP (e.g. Bayern, Sachsen), FFF/BBBB/UUUP (NRW)
        (r"\b\d{2,3}/\d{3,4}/\d{4,5}\b", "[TAX_ID]"),
        // Any number after a keyword, e.g. "Steuernummer: 7890123456",
        // "St.-Nr. 12 345 678 901", "<TaxpayerID>7331921667168"
        (
            r#"(?P<label>\b(?i:Steuer-?(?:nummer|nr\.?)|St\.-?Nr\.?|Steuer-?(?:identifikationsnummer|ID)|Tax\s?ID|Taxpayer\s?ID)[\s:*"'>=]{0,6})[0-9][0-9A-Z/ ]{8,14}[0-9]\b"#,
            "${label}[TAX_ID]",
        ),
        // Candidates for further formats:
        // (r"\b0\d{2} \d{3} \d{5}\b", "[TAX_ID]"), // Hessen 0FF BBB UUUUP, but clashes with phone numbers like 030 123 45678
        // (r"\b[1-9]\d{12}\b", "[TAX_ID]"),        // unified federal 13-digit format, but clashes with ID card numbers
        // (r"\b\d{2} \d{3} \d{3} \d{3}\b", "[TAX_ID]"), // Steuer-ID (IdNr) 11 digits, grouped, without keyword
        // (r"(?P<label>\bTIN[\s:]{0,3})[0-9A-Z]{9,13}\b", "${label}[TAX_ID]"), // English "TIN" keyword
    ]
    .iter()
    .map(|(p, r)| (Regex::new(p).expect("invalid tax ID regex"), *r))
    .collect()
});

// IP address patterns, as (regex, replacement) pairs. These run before the date
// and time patterns, which would otherwise match parts of addresses such as
// 10.1.23.4 (dd.mm.yy) or a893:12:34:... (hh:mm).
static IP_PATTERNS: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    let octet = r"(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)"; // 0-255
    let ipv4 = format!(r"(?:{octet}\.){{3}}{octet}");
    let hex = r"[0-9A-Fa-f]{1,4}"; // one IPv6 group
    [
        // IPv4-mapped IPv6, e.g. ::ffff:192.168.1.1
        (format!(r"(?i:::ffff:){ipv4}\b"), "[IP]"),
        // IPv4 with each octet 0-255, e.g. 51.191.200.88. Not followed by "."
        // and a letter or digit, to leave longer dotted numbers intact, such as
        // social security numbers 72.16.10.63.B04.6. The character after the
        // address is captured and put back. Known gap: 1.2.3.4.5 becomes 1.[IP].
        (format!(r"\b{ipv4}(?P<post>[^.0-9A-Za-z]|\.(?:[^0-9A-Za-z]|$)|$)"), "[IP]${post}"),
        // IPv6 without "::", 4-8 groups, e.g. a893:d59e:e7ef:4ec3:520d:1710:207:2b70.
        // Fewer than 8 groups is not a valid address, but occurs in the data as
        // truncated addresses. 3 groups are not matched, as they clash with hh:mm:ss.
        (format!(r"\b(?:{hex}:){{3,7}}{hex}\b"), "[IP]"),
        // IPv6 compressed with "::" in the middle or at the end, e.g. fe80::1:23, fe80::
        (format!(r"\b(?:{hex}:){{1,6}}(?::{hex}){{1,6}}\b|\b(?:{hex}:){{1,7}}:"), "[IP]"),
        // IPv6 starting with "::", e.g. ::1; not directly after a letter or
        // digit, to leave e.g. std::abc in code intact
        (format!(r"(?P<pre>^|[^0-9A-Za-z:])::(?:{hex}:){{0,6}}{hex}\b"), "${pre}[IP]"),
        // Candidates for further formats:
        // (format!(r"\b{ipv4}/\d{{1,2}}\b"), "[IP]"),                     // IPv4 with CIDR prefix, e.g. 10.0.0.0/8
        // (format!(r"\b(?:{hex}:){{2}}{hex}\b"), "[IP]"),                 // truncated 3-group IPv6, e.g. c6a4:93fd:63 (clashes with hh:mm:ss)
        // (format!(r"(?i)\b(?:[0-9a-f]{{2}}[:-]){{5}}[0-9a-f]{{2}}\b"), "<MAC>"), // MAC address, e.g. 00:1A:2B:3C:4D:5E
        //                                                                // (run before IPv6, which otherwise labels colon-form MACs as [IP])
    ]
    .into_iter()
    .map(|(p, r)| (Regex::new(&p).expect("invalid IP regex"), r))
    .collect()
});

// Social security number patterns. These run first, since the IP and date
// patterns would otherwise match parts of them, e.g. 2.64.04 as dd.mm.yy.
// Groups may be separated by "-", "." or " ", or not at all.
static SOCIAL_NUMBER_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        // German Rentenversicherungsnummer: area (2), birth date ddmmyy (6), initial
        // of birth name, serial (2), check digit, e.g. 41-27-05-46-K77-1, 35080239S156
        r"\b\d{2}[-. ]?\d{2}[-. ]?\d{2}[-. ]?\d{2}[-. ]?[A-Z]\d{2}[-. ]?\d\b",
        // Swiss AHV number: 756 + 10 digits, e.g. 756.8319.4907.37, 7568665471296
        r"\b756[-. ]?\d{4}[-. ]?\d{4}[-. ]?\d{2}\b",
        // French NIR: sex, yy, mm, place (5), serial (3), key (2), e.g. 2.64.04.37968.005.13
        r"\b[12][-. ]?\d{2}[-. ]?\d{2}[-. ]?\d{5}[-. ]?\d{3}[-. ]?\d{2}\b",
        // US SSN with consistent separators, e.g. 669-09-2502, 642 50 3478, 255.57.3442
        r"\b\d{3}-\d{2}-\d{4}\b|\b\d{3}\.\d{2}\.\d{4}\b|\b\d{3} \d{2} \d{4}\b",
        // Name-based format found in the data: 3+3 letters of first and last name,
        // digits and letters, e.g. Lye Ras 11 B 54 7 ZUM, CarFol25C417AIP
        r"\b[A-Z][a-z]{2}[-. ]?[A-Z][a-z]{2}[-. ]?\d{2}[-. ]?[A-Z][-. ]?\d{2}[-. ]?\d[-. ]?[A-Z]{3}\b",
        // Candidates for further formats:
        // r"\b\d{3}[-. ]\d{3}[-. ]\d{4}\b", // 3-3-4 digits, e.g. 917-346-0382 (clashes with phone numbers)
        // r"(?i)(?:sozialversicherungsnummer|social_?number|ssn)[\s:*\x22'>=]{0,6}\d{9,15}\b",
        //                                   // bare 9-15 digits after a keyword, e.g. "SocialNumber": "834409891"
        //                                   // (replace the whole match, or add a label group as in TAX_ID_PATTERNS)
    ]
    .iter()
    .map(|p| Regex::new(p).expect("invalid social security number regex"))
    .collect()
});

// Phone number patterns. These run last, after dates, IPs, IBANs etc. have been
// replaced, since those would otherwise be taken for phone numbers. Digit groups
// may be separated by "-", ".", " " or "/", in any mix.
static PHONE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        // International with "+", optionally with "(0)", e.g. +49 30 1234567,
        // +49-57 643 4205, +49 (0)30 1234567, +4165941 4831
        r"\+\d{1,3}[-. /]?(?:\(0\)[-. /]?)?\d{1,5}(?:[-. /]?\d{2,9}){1,4}\b",
        // National with leading 0 (or 00 for international), with at least one
        // separator, e.g. 0165-57634430, 030 1234567, 075-656 6263, 0015.43664924.
        // Bare digit runs like 0745848267 are not matched: in the data these are
        // ID card, passport or driver's license numbers.
        r"\b0\d{2,9}[-. /]\d{3,9}(?:[-. /]\d{2,9}){0,3}\b",
        // Area code in parentheses, e.g. (030) 1234567, (0165) 576-34430
        r"\(0\d{2,5}\)[-. /]?\d{3,9}(?:[-. /]?\d{2,9}){0,3}\b",
        // Candidates for further formats:
        // r"\b0\d{9,11}\b",                        // bare mobile/landline, e.g. 01701234567 (clashes with ID numbers in the data)
        // r"\b[1-9]\d{2}[-. ]\d{3}[-. ]\d{4}\b",   // 3-3-4 digits without leading 0, e.g. 917-346-0382 (US style)
        // r"\(\d{3}\)\s?\d{3}[-. ]\d{4}\b",       // US style with area code in parentheses, e.g. (212) 555-1234
        // r"(?i)(?:tel|telefon|fon|mobil|handy|fax)\.?[\s:*\x22'>=]{0,6}\d[\d\-. /]{5,18}\d\b",
        //                                        // any digits after a keyword, e.g. "Tel.: 12345678"
        //                                        // (replace the whole match, or add a label group as in TAX_ID_PATTERNS)
    ]
    .iter()
    .map(|p| Regex::new(p).expect("invalid phone regex"))
    .collect()
});

// Email address patterns. These run first, since digits in the local part
// could otherwise be taken for dates, phone numbers etc., e.g. 12.05.1990@gmail.com.
static EMAIL_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        // local part @ domain with a top-level domain of 2+ letters, e.g.
        // asschmoll@tutanota.com, 05pierre-hugues.dunkl@tutanota.com. \w is
        // Unicode-aware, so local parts like dünhaupt@aol.com are covered.
        r"[\w.+%-]+@[\w-]+(?:\.[\w-]+)*\.\p{L}{2,}\b",
        // Candidates for further formats:
        // r"[\w.+%-]+@(?:tutanota|gmail|yahoo|aol|outlook|hotmail|protonmail|gmx|web)\b",
        //                                          // truncated, without TLD, e.g. 41giulyan@tutanota
        // r"(?i)[\w.+%-]+\s?(?:\(at\)|\[at\]| at )\s?[\w-]+\s?(?:\(dot\)|\[dot\]|\.| dot )\s?\p{L}{2,}\b",
        //                                          // obfuscated, e.g. max (at) example (dot) com
        // r"[\w.+%-]+@\[\d{1,3}(?:\.\d{1,3}){3}\]", // IP address as domain, e.g. user@[192.168.1.1]
    ]
    .iter()
    .map(|p| Regex::new(p).expect("invalid email regex"))
    .collect()
});

// Postal address patterns, as (regex, replacement) pairs. Parts of an address
// such as a city or a house number cannot be recognized on their own, so these
// either look for a label (keeping it in the output), or for a street name with
// a typical German suffix. They run last, so values that are already replaced,
// e.g. <PHONE>, end a match; the order within the list matters.
static ADDRESS_PATTERNS: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    [
        // Value after a label, up to the end of the field, e.g. "Straße: Klappenweg",
        // "postcode": "52477", <city>Magdeburg</city>, **Bundesland:** Bayern
        (
            r#"(?P<label>\b(?i:straße|strasse|street|gebäude|building(?:_number)?|hausnummer|postleitzahl|plz|post_?code|postal[_ ]?code|stadt|city|ort|bundesland|state|zweite adresse|zweitadresse|sekundäre adresse|zusätzliche adresse|nebenadresse|zusatzadresse|secondary[_ ]?address|sec_?address)[ \t*"']*[:=>][ \t*"']*)[^"'<>\n*,]*[^"'<>\n*,\s]"#,
            "${label}[ADDRESS]",
        ),
        // Street ending in -straße/-strasse/-str., with optional house number,
        // e.g. Hauptstraße 22, Maria-Montessori-Straße, Ratheimer Straße 5a
        (
            r"\b\p{Lu}[\p{L}-]*(?:straße|strasse|str\.|-Straße|-Str\.|er Straße|er Str\.)(?: \d{1,4}(?: ?[a-z])?\b)?",
            "[ADDRESS]",
        ),
        // Street with another suffix, only with house number, since words like
        // "Heimweg" or "Arbeitsplatz" are common, e.g. Schauinslandweg 8, Am Lindenplatz 3
        // Added: hof
        (
            r"\b\p{Lu}[\p{L}-]*(?:weg|hof|gasse|platz|allee|ring|damm|ufer|chaussee|pfad|steig|stieg|promenade|kamp|wall|graben|markt) \d{1,4}(?: ?[a-z])?\b",
            "[ADDRESS]",
        ),
        // Postcode and city directly after a street, e.g. "[ADDRESS], 10115 Berlin",
        // "[ADDRESS] D-79117 Freiburg im Breisgau"
        (
            r"[ADDRESS],? (?:D-)?\d{5} \p{Lu}[\p{L}-]+(?: (?:im|am|an der|ob der|vor der) \p{Lu}[\p{L}-]+| ?\(\p{Lu}\p{L}+\)|/\p{Lu}\p{L}+)?",
            "[ADDRESS]",
        ),
        // Candidates for further formats:
        // (r"\bD-\d{5} \p{Lu}[\p{L}-]+", "[ADDRESS]"),         // postcode with country prefix, e.g. D-10115 Berlin
        // (r"\b\d{5} \p{Lu}[\p{L}-]+", "[ADDRESS]"),           // postcode + city alone (clashes with e.g. "12345 Datum")
        // (r"\b(?:Am|An der|Im|Zum|Zur|Auf dem) \p{Lu}[\p{L}-]+ \d{1,4}(?: ?[a-z])?\b", "[ADDRESS]"),
        //                                                     // prepositional street names, e.g. Im Jägeringshof 12
        // (r"\b(?:Rue|Avenue|Boulevard|Chemin|Impasse|Calle|Avenida|Camino|Via|Piazza)(?: (?:de|des|du|la|le|del|di|d'))* \p{Lu}[\p{L}-]+(?: \p{Lu}[\p{L}-]+)*", "[ADDRESS]"),
        //                                                     // French/Spanish/Italian, e.g. Rue des Écoles
        // (r"\b\p{Lu}\p{L}+ (?:Road|Street|Lane|Avenue|Drive)\b", "[ADDRESS]"), // English, e.g. Rectory Road
        // (r"\b(?:Apt|Apartment|Wohnung|Etage|Floor|Suite|Unit|Postfach|Box) \d{1,4}\b", "[ADDRESS]"),
        //                                                     // secondary address, e.g. Apt 12, Postfach 123
    ]
    .iter()
    .map(|(p, r)| (Regex::new(p).expect("invalid address regex"), *r))
    .collect()
});

// Driver's license patterns, as (regex, replacement) pairs. These run right after
// the email patterns, so that labelled values are not taken for other numbers.
static DRIVER_LICENSE_PATTERNS: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    [
        // Structured format found in the data: letter + digit, 2 digits, 7 letters
        // or digits, check digit, separated by "-", "." or " " or not at all,
        // e.g. T5-46-HLFZFTF-2, B2 18 CJYFTWD 9, Q865UNULDLM2
        (r"\b[A-Z]\d[-. ]?\d{2}[-. ]?[A-Z0-9]{7}[-. ]?\d\b", "[DRIVERLICENSE]"),
        // Any number containing a digit after a label, e.g. "Führerschein: M41348406",
        // "driver_license": "4795321223". Such values are not matched without a
        // label, since e.g. letter + 8 digits is mostly an ID card number in the data.
        // The label is kept; "führerschein" occurs in doubly escaped JSON.
        (
            r#"(?P<label>\b(?i:f(?:ü|\\u00fc)hrerschein(?:nummer|-?nr\.?|-nummer)?|fahrerlaubnis(?:nummer)?|fahrerlizenz|driver'?s?[_ ]?licen[cs]e(?:[_ ]?(?:number|no\.?|id))?|driverlicense)[ \t*"':=>]{1,6})(?:[A-Z][A-Z.\-]*\d[A-Z0-9.\-]*[A-Z0-9]|\d[A-Z0-9.\-]{3,}[A-Z0-9])\b"#,
            "${label}[DRIVERLICENSE]",
        ),
        // Candidates for further formats:
        // (r"\b[A-Z0-9]\d{2}[A-Z0-9]{6}\d[A-Z0-9]\b", "[DRIVERLICENSE]"), // German EU card (11 chars), e.g. B072RRE2I55
        // (r"\b[A-Z]\d{3}-\d{4}-\d{4}\b", "[DRIVERLICENSE]"),            // US style, e.g. A123-4567-8901
        // (r"\b[A-Z]{5}\d{6}[A-Z0-9]{5}\b", "[DRIVERLICENSE]"),          // UK (16 chars), e.g. MORGA657054SM9IJ
    ]
    .iter()
    .map(|(p, r)| (Regex::new(p).expect("invalid driver's license regex"), *r))
    .collect()
});

// ID card patterns, as (regex, replacement) pairs. These run right after the
// driver's license patterns, which take labelled license numbers of the same
// shape first (e.g. "Führerschein: M41348406").
static ID_CARD_PATTERNS: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    [
        // Formats found in the data, e.g. RD7407623, Y98158610, VF58428VF,
        // QEB503078G, Z2609487476600, 13960813A, O9745471G
        (
            r"\b(?:[A-Z]{2}\d{7}|[A-Z]\d{8}|[A-Z]{2}\d{5}[A-Z]{2}|[A-Z]{3}\d{6}[A-Z]|[A-Z]\d{13}|\d{8}[A-Z]|[A-Z]\d{7}[A-Z])\b",
            "[IDCARD]",
        ),
        // Any number containing a digit after a label, e.g. "Ausweisnummer: W5900073",
        // "id_card": "1129843465". Letter + 7 digits is not matched without a
        // label, since it is often a passport number in the data. The label is kept.
        (
            r#"(?P<label>\b(?i:(?:personal)?ausweis(?:nummer|-?nr\.?|-nummer|karte|dokument)?|id[-_ ]?kart(?:e|en-?nummer)|id[-_ ]?card(?:[-_ ]?(?:number|no\.?|nr\.?))?|identity[-_ ]card(?:[-_ ]number)?)[ \t*"':=>]{1,6})(?:[A-Z][A-Z.\-]*\d[A-Z0-9.\-]*[A-Z0-9]|\d[A-Z0-9.\-]{3,}[A-Z0-9])\b"#,
            "${label}[IDCARD]",
        ),
        // Candidates for further formats:
        // (r"\b[CFGHJKLMNPRTVWXYZ][CFGHJKLMNPRTVWXYZ0-9]{8}\d?\b", "[IDCARD]"),
        //                                   // real German Personalausweis (9 chars + optional check digit), e.g. L01X00T47
        //                                   // (may match 9-letter words in capitals)
        // (r"\b[A-Z]\d{7}\b", "[IDCARD]"), // letter + 7 digits without label (clashes with passport numbers)
        // (r"(?i)(?:patient|student|kunden|mitglieds)[-_ ]?(?:id|nummer|nr\.?)[ \t*\x22':=>]{1,6}[A-Z0-9-]{4,}\b", "[IDCARD]"),
        //                                   // other personal IDs, e.g. "patient_id": "P12345" (replace the whole match)
    ]
    .iter()
    .map(|(p, r)| (Regex::new(p).expect("invalid ID card regex"), *r))
    .collect()
});

// Identify certain PII elements
fn redact(text: &str) -> Result<String> {

    // Make a copy of the input
    let mut result = String::from(text);

    // Look for email address patterns
    for re in EMAIL_PATTERNS.iter() {
        result = re.replace_all(&result, "[EMAIL]").into_owned();
    }

    // Look for driver's license patterns
    for (re, replacement) in DRIVER_LICENSE_PATTERNS.iter() {
        result = re.replace_all(&result, *replacement).into_owned();
    }

    // Look for ID card patterns
    for (re, replacement) in ID_CARD_PATTERNS.iter() {
        result = re.replace_all(&result, *replacement).into_owned();
    }

    // Look for Steuernummer patterns; before social security numbers, which
    // would otherwise take labelled values like "Steuernummer: 67010143B907"
    for (re, replacement) in TAX_ID_PATTERNS.iter() {
        result = re.replace_all(&result, *replacement).into_owned();
    }

    // Look for social security number patterns
    for re in SOCIAL_NUMBER_PATTERNS.iter() {
        result = re.replace_all(&result, "[SOCIALNUMBER]").into_owned();
    }

    // Look for IP address patterns
    for (re, replacement) in IP_PATTERNS.iter() {
        result = re.replace_all(&result, *replacement).into_owned();
    }

    // Look for IBAN patterns
    for re in IBAN_PATTERNS.iter() {
        result = re.replace_all(&result, "[IBAN]").into_owned();
    }

    // Look for date patterns
    for re in DATE_PATTERNS.iter() {
        result = re.replace_all(&result, "[DATE]").into_owned();
    }

    // Look for time patterns
    for (re, replacement) in TIME_PATTERNS.iter() {
        result = re.replace_all(&result, *replacement).into_owned();
    }

    // Look for phone number patterns
    for re in PHONE_PATTERNS.iter() {
        result = re.replace_all(&result, "[PHONE]").into_owned();
    }

    // Look for postal address patterns
    for (re, replacement) in ADDRESS_PATTERNS.iter() {
        result = re.replace_all(&result, *replacement).into_owned();
    }

    // Return altered string
    Ok(result)
}
