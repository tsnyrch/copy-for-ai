/// ISO 13616 length and mod-97, including common European account formats.
pub(crate) fn iban_length(value: &str) -> Option<usize> {
    let expected = match &value[..2] {
        "CZ" | "SK" | "ES" | "SE" | "RO" => 24,
        "DE" | "GB" | "IE" | "BG" => 22,
        "AT" | "EE" | "LT" | "LU" => 20,
        "BE" => 16,
        "NL" | "DK" | "FI" => 18,
        "CH" | "LI" => 21,
        "FR" | "IT" | "GR" | "MC" => 27,
        "PL" | "HU" | "CY" => 28,
        "PT" => 25,
        "NO" => 15,
        _ => return None,
    };
    let mut compact = Vec::with_capacity(expected);
    let mut end = 0;
    for (index, byte) in value.bytes().enumerate() {
        if byte != b' ' {
            compact.push(byte);
        }
        if compact.len() == expected {
            end = index + 1;
            break;
        }
    }
    if compact.len() != expected || value[end..].bytes().next().is_some_and(|b| b != b' ') {
        return None;
    }
    let mut remainder = 0_u32;
    for &byte in compact[4..].iter().chain(&compact[..4]) {
        remainder = match byte {
            b'0'..=b'9' => (remainder * 10 + u32::from(byte - b'0')) % 97,
            b'A'..=b'Z' => (remainder * 100 + u32::from(byte - b'A' + 10)) % 97,
            _ => return None,
        };
    }
    (remainder == 1).then_some(end)
}

/// Czech birth number: real date, sex/month offsets and historical checksum.
pub(crate) fn valid_czech_id(value: &str) -> bool {
    let digits: Vec<u32> = value
        .bytes()
        .filter(|b| b.is_ascii_digit())
        .map(|b| u32::from(b - b'0'))
        .collect();
    if !matches!(digits.len(), 9 | 10) {
        return false;
    }
    let short_year = digits[0] * 10 + digits[1];
    let year = if digits.len() == 9 || short_year >= 54 {
        1900 + short_year
    } else {
        2000 + short_year
    };
    if digits.len() == 9 && short_year >= 54 {
        return false;
    }
    let mut month = digits[2] * 10 + digits[3];
    if month >= 50 {
        month -= 50;
    }
    if month > 20 && year >= 2004 {
        month -= 20;
    }
    let day = digits[4] * 10 + digits[5];
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => return false,
    };
    if day == 0 || day > max_day {
        return false;
    }
    if digits.len() == 9 {
        return true;
    }
    let first_nine = digits[..9].iter().fold(0, |n, d| n * 10 + d);
    let remainder = first_nine % 11;
    remainder == digits[9] || (year < 1985 && remainder == 10 && digits[9] == 0)
}
