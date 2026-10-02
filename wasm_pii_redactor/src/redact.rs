// Function to redact PII, replacing it with tokenized strings

//use std::io::Result;
use std::sync::LazyLock;

use regex::Regex;

// Date patterns, compiled once on first use. Each pattern is surrounded by \b so that e.g. parts
// of IP addresses or longer digit strings are not matched.  Note that these could be more strict
// by checking numeric values against expected ranges.
static DATE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"\b\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:?\d{2})?", // 1943-06-04T00:00:00
        r"\b\d{1,2}/\d{1,2}/\d{4}\b",   // dd/mm/yyyy
        r"\b\d{1,2}\.\d{1,2}\.\d{4}\b", // dd.mm.yyyy
        r"\b\d{4}-\d{1,2}-\d{1,2}\b",   // yyyy-mm-dd
        r"\b\d{1,2}\.\d{1,2}\.\d{2}\b", // dd.mm.yy       e.g. 05.01.64
        r"\b\d{1,2}-\d{1,2}-\d{4}\b",   // dd-mm-yyyy     e.g. 05-01-2064
        r"\b\d{4}/\d{1,2}/\d{1,2}\b",   // yyyy/mm/dd     e.g. 2064/01/05
        r"\b\d{1,2}\.\s?(Januar|Februar|März|April|Mai|Juni|Juli|August|September|Oktober|November|Dezember)\s\d{4}\b",
                                        // dd. Monat yyyy e.g. 5. Januar 2064
        r"\b\d{1,2}\.\s?(Jan|Feb|Mär|Apr|Mai|Jun|Jul|Aug|Sep|Okt|Nov|Dez)\.?\s\d{4}\b",
                                        // dd. Mon yyyy   e.g. 5. Jan. 2064
        r"\b(Januar|Februar|März|April|Mai|Juni|Juli|August|September|Oktober|November|Dezember)\s\d{4}\b",
                                        // Monat yyyy     e.g. Januar 2064
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
    ]
    .iter()
    .map(|(p, r)| (Regex::new(p).expect("invalid time regex"), *r))
    .collect()
});

// IBAN patterns: 2-letter country code, 2 check digits, then 11-30 alphanumeric
// characters (total length 15-34). TODO: verify the checksum.
static IBAN_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"\b[A-Z]{2}\d{2}[A-Z0-9]{11,30}\b", // compact       e.g. DE89370400440532013000
        r"\b[A-Z]{2}\d{2}(?: [A-Z0-9]{4}){2,7}(?: [A-Z0-9]{1,3})?\b", // grouped e.g. DE89 3704 0044 0532 0130 00
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
        // "postcode": "52477", <city>Magdeburg</city>, **Bundesland:** Bayern,
        // <strong>Stadt:</strong> Halle (an HTML tag may follow the separator)
        (
            r#"(?P<label>\b(?i:straße|strasse|street|gebäude|building(?:_number)?|hausnummer|postleitzahl|plz|post_?code|postal[_ ]?code|stadt|city|ort|bundesland|state|zweite adresse|zweitadresse|sekundäre adresse|zusätzliche adresse|nebenadresse|zusatzadresse|secondary[_ ]?address|sec_?address)[ \t*"']*[:=>][ \t*"']*(?:</?[a-z]+>[ \t]*)?)[^"'<>\n*,]*[^"'<>\n*,\s]"#,
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
            r"\[ADDRESS\],? (?:D-)?\d{5} \p{Lu}[\p{L}-]+(?: (?:im|am|an der|ob der|vor der) \p{Lu}[\p{L}-]+| ?\(\p{Lu}\p{L}+\)|/\p{Lu}\p{L}+)?",
            "[ADDRESS]",
        ),
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
    ]
    .iter()
    .map(|(p, r)| (Regex::new(p).expect("invalid ID card regex"), *r))
    .collect()
});

// Take a string, identify various PII elements, and replace them with tokens. Uses lots of regular
// expressions, so very fast.
pub fn redact(text: &str) -> String {

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
    result
}
