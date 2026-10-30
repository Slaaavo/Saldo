pub fn strip_accents(text: &str) -> String {
    let mut stripped = String::with_capacity(text.len());
    for c in text.chars() {
        if ('\u{0300}'..='\u{036f}').contains(&c) {
            continue;
        }
        let mut lower = c.to_lowercase();
        let base = match (lower.next(), lower.next()) {
            (Some(lower), None) => latin_base(lower),
            _ => None,
        };
        match base {
            Some(base) if c.is_uppercase() => stripped.push_str(&base.to_uppercase()),
            Some(base) => stripped.push_str(base),
            None => stripped.push(c),
        }
    }
    stripped
}

fn latin_base(c: char) -> Option<&'static str> {
    Some(match c {
        'à'..='å' | 'ā' | 'ă' | 'ą' => "a",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'ď' | 'đ' => "d",
        'è'..='ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'ĥ' | 'ħ' => "h",
        'ì'..='ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'ĵ' => "j",
        'ķ' => "k",
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
        'ñ' | 'ń' | 'ņ' | 'ň' => "n",
        'ò'..='ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'ś' | 'ŝ' | 'ş' | 'š' => "s",
        'ţ' | 'ť' | 'ŧ' => "t",
        'ù'..='ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'ŵ' => "w",
        'ý' | 'ÿ' | 'ŷ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        'ß' => "ss",
        'æ' => "ae",
        'œ' => "oe",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::strip_accents;

    #[test]
    fn accents_go_and_case_stays() {
        assert_eq!(strip_accents("ŠKODA Káva"), "SKODA Kava");
        assert_eq!(strip_accents("ĽUBOŠ Ďurčík ÔŔ"), "LUBOS Durcik OR");
        assert_eq!(strip_accents("čaj ČAJ"), "caj CAJ");
    }

    #[test]
    fn combining_marks_and_ligatures_fold_too() {
        assert_eq!(strip_accents("S\u{030C}KODA"), "SKODA");
        assert_eq!(strip_accents("Straße"), "Strasse");
        assert_eq!(strip_accents("Æsop"), "AEsop");
    }

    #[test]
    fn text_without_accents_is_unchanged() {
        let text = r"KAVA 1KG \d+ (x|y) 23 %";
        assert_eq!(strip_accents(text), text);
    }
}
