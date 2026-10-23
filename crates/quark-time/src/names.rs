pub struct Names {
    pub months: [&'static str; 12],
    pub months_short: [&'static str; 12],
    pub weekdays: [&'static str; 7],
    pub weekdays_short: [&'static str; 7],
}

const LANGUAGES: &[(&str, Names)] = &[
    (
        "en",
        Names {
            months: [
                "January",
                "February",
                "March",
                "April",
                "May",
                "June",
                "July",
                "August",
                "September",
                "October",
                "November",
                "December",
            ],
            months_short: [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ],
            weekdays: [
                "Monday",
                "Tuesday",
                "Wednesday",
                "Thursday",
                "Friday",
                "Saturday",
                "Sunday",
            ],
            weekdays_short: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
        },
    ),
    (
        "de",
        Names {
            months: [
                "Januar",
                "Februar",
                "März",
                "April",
                "Mai",
                "Juni",
                "Juli",
                "August",
                "September",
                "Oktober",
                "November",
                "Dezember",
            ],
            months_short: [
                "Jan.", "Feb.", "März", "Apr.", "Mai", "Juni", "Juli", "Aug.", "Sept.", "Okt.",
                "Nov.", "Dez.",
            ],
            weekdays: [
                "Montag",
                "Dienstag",
                "Mittwoch",
                "Donnerstag",
                "Freitag",
                "Samstag",
                "Sonntag",
            ],
            weekdays_short: ["Mo.", "Di.", "Mi.", "Do.", "Fr.", "Sa.", "So."],
        },
    ),
    (
        "fr",
        Names {
            months: [
                "janvier",
                "février",
                "mars",
                "avril",
                "mai",
                "juin",
                "juillet",
                "août",
                "septembre",
                "octobre",
                "novembre",
                "décembre",
            ],
            months_short: [
                "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.",
                "nov.", "déc.",
            ],
            weekdays: [
                "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche",
            ],
            weekdays_short: ["lun.", "mar.", "mer.", "jeu.", "ven.", "sam.", "dim."],
        },
    ),
    (
        "es",
        Names {
            months: [
                "enero",
                "febrero",
                "marzo",
                "abril",
                "mayo",
                "junio",
                "julio",
                "agosto",
                "septiembre",
                "octubre",
                "noviembre",
                "diciembre",
            ],
            months_short: [
                "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic",
            ],
            weekdays: [
                "lunes",
                "martes",
                "miércoles",
                "jueves",
                "viernes",
                "sábado",
                "domingo",
            ],
            weekdays_short: ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"],
        },
    ),
    (
        "it",
        Names {
            months: [
                "gennaio",
                "febbraio",
                "marzo",
                "aprile",
                "maggio",
                "giugno",
                "luglio",
                "agosto",
                "settembre",
                "ottobre",
                "novembre",
                "dicembre",
            ],
            months_short: [
                "gen", "feb", "mar", "apr", "mag", "giu", "lug", "ago", "set", "ott", "nov", "dic",
            ],
            weekdays: [
                "lunedì",
                "martedì",
                "mercoledì",
                "giovedì",
                "venerdì",
                "sabato",
                "domenica",
            ],
            weekdays_short: ["lun", "mar", "mer", "gio", "ven", "sab", "dom"],
        },
    ),
    (
        "pt",
        Names {
            months: [
                "janeiro",
                "fevereiro",
                "março",
                "abril",
                "maio",
                "junho",
                "julho",
                "agosto",
                "setembro",
                "outubro",
                "novembro",
                "dezembro",
            ],
            months_short: [
                "jan.", "fev.", "mar.", "abr.", "mai.", "jun.", "jul.", "ago.", "set.", "out.",
                "nov.", "dez.",
            ],
            weekdays: [
                "segunda-feira",
                "terça-feira",
                "quarta-feira",
                "quinta-feira",
                "sexta-feira",
                "sábado",
                "domingo",
            ],
            weekdays_short: ["seg.", "ter.", "qua.", "qui.", "sex.", "sáb.", "dom."],
        },
    ),
    (
        "nl",
        Names {
            months: [
                "januari",
                "februari",
                "maart",
                "april",
                "mei",
                "juni",
                "juli",
                "augustus",
                "september",
                "oktober",
                "november",
                "december",
            ],
            months_short: [
                "jan", "feb", "mrt", "apr", "mei", "jun", "jul", "aug", "sep", "okt", "nov", "dec",
            ],
            weekdays: [
                "maandag",
                "dinsdag",
                "woensdag",
                "donderdag",
                "vrijdag",
                "zaterdag",
                "zondag",
            ],
            weekdays_short: ["ma", "di", "wo", "do", "vr", "za", "zo"],
        },
    ),
    (
        "pl",
        Names {
            months: [
                "styczeń",
                "luty",
                "marzec",
                "kwiecień",
                "maj",
                "czerwiec",
                "lipiec",
                "sierpień",
                "wrzesień",
                "październik",
                "listopad",
                "grudzień",
            ],
            months_short: [
                "sty", "lut", "mar", "kwi", "maj", "cze", "lip", "sie", "wrz", "paź", "lis", "gru",
            ],
            weekdays: [
                "poniedziałek",
                "wtorek",
                "środa",
                "czwartek",
                "piątek",
                "sobota",
                "niedziela",
            ],
            weekdays_short: ["pon.", "wt.", "śr.", "czw.", "pt.", "sob.", "niedz."],
        },
    ),
    (
        "ru",
        Names {
            months: [
                "январь",
                "февраль",
                "март",
                "апрель",
                "май",
                "июнь",
                "июль",
                "август",
                "сентябрь",
                "октябрь",
                "ноябрь",
                "декабрь",
            ],
            months_short: [
                "янв.",
                "февр.",
                "март",
                "апр.",
                "май",
                "июнь",
                "июль",
                "авг.",
                "сент.",
                "окт.",
                "нояб.",
                "дек.",
            ],
            weekdays: [
                "понедельник",
                "вторник",
                "среда",
                "четверг",
                "пятница",
                "суббота",
                "воскресенье",
            ],
            weekdays_short: ["пн", "вт", "ср", "чт", "пт", "сб", "вс"],
        },
    ),
    (
        "tr",
        Names {
            months: [
                "Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül",
                "Ekim", "Kasım", "Aralık",
            ],
            months_short: [
                "Oca", "Şub", "Mar", "Nis", "May", "Haz", "Tem", "Ağu", "Eyl", "Eki", "Kas", "Ara",
            ],
            weekdays: [
                "Pazartesi",
                "Salı",
                "Çarşamba",
                "Perşembe",
                "Cuma",
                "Cumartesi",
                "Pazar",
            ],
            weekdays_short: ["Pzt", "Sal", "Çar", "Per", "Cum", "Cmt", "Paz"],
        },
    ),
    (
        "sv",
        Names {
            months: [
                "januari",
                "februari",
                "mars",
                "april",
                "maj",
                "juni",
                "juli",
                "augusti",
                "september",
                "oktober",
                "november",
                "december",
            ],
            months_short: [
                "jan.", "feb.", "mars", "apr.", "maj", "juni", "juli", "aug.", "sep.", "okt.",
                "nov.", "dec.",
            ],
            weekdays: [
                "måndag", "tisdag", "onsdag", "torsdag", "fredag", "lördag", "söndag",
            ],
            weekdays_short: ["mån", "tis", "ons", "tors", "fre", "lör", "sön"],
        },
    ),
    (
        "cs",
        Names {
            months: [
                "leden",
                "únor",
                "březen",
                "duben",
                "květen",
                "červen",
                "červenec",
                "srpen",
                "září",
                "říjen",
                "listopad",
                "prosinec",
            ],
            months_short: [
                "led", "úno", "bře", "dub", "kvě", "čvn", "čvc", "srp", "zář", "říj", "lis", "pro",
            ],
            weekdays: [
                "pondělí",
                "úterý",
                "středa",
                "čtvrtek",
                "pátek",
                "sobota",
                "neděle",
            ],
            weekdays_short: ["po", "út", "st", "čt", "pá", "so", "ne"],
        },
    ),
];

pub fn find(locale: &str) -> Option<&'static Names> {
    let language = locale
        .split(['-', '_'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    LANGUAGES
        .iter()
        .find(|(code, _)| *code == language)
        .map(|(_, names)| names)
}

pub fn languages() -> impl Iterator<Item = &'static str> {
    LANGUAGES.iter().map(|(code, _)| *code)
}

pub fn localize(pattern: &str, names: &Names, month: usize, weekday: usize) -> String {
    let mut output = String::with_capacity(pattern.len());
    let mut characters = pattern.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '%' {
            output.push(character);
            continue;
        }
        if characters.peek() == Some(&'%') {
            characters.next();
            output.push_str("%%");
            continue;
        }
        let mut flags = String::new();
        while let Some(&flag) = characters.peek() {
            if matches!(flag, '-' | '_' | '0' | '^' | '#') {
                flags.push(flag);
                characters.next();
            } else {
                break;
            }
        }
        let replacement = match characters.peek() {
            Some('B') => Some(names.months[month]),
            Some('b') | Some('h') => Some(names.months_short[month]),
            Some('A') => Some(names.weekdays[weekday]),
            Some('a') => Some(names.weekdays_short[weekday]),
            _ => None,
        };
        match replacement {
            Some(name) => {
                characters.next();
                let name = if flags.contains('^') {
                    name.to_uppercase()
                } else {
                    name.to_owned()
                };
                output.push_str(&name.replace('%', "%%"));
            }
            None => {
                output.push('%');
                output.push_str(&flags);
            }
        }
    }
    output
}
