use crate::validate::validate_timezone;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionGuess {
    pub keymap: String,
    pub locale: String,
    pub timezone: String,
}

pub fn detect_region() -> Option<RegionGuess> {
    let output = Command::new("curl")
        .args(["-fsS", "--max-time", "4", "https://ipapi.co/json/"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let body = String::from_utf8(output.stdout).ok()?;
    guess_from_geo(&body)
}

pub fn guess_from_geo(body: &str) -> Option<RegionGuess> {
    let country = json_string(body, "country_code")?;
    let timezone = json_string(body, "timezone").unwrap_or_else(|| "UTC".to_string());
    Some(from_country(&country, &timezone))
}

pub fn from_country(country: &str, timezone: &str) -> RegionGuess {
    let (keymap, locale, fallback) = match country.to_ascii_uppercase().as_str() {
        "IT" | "SM" | "VA" => ("it", "it_IT.UTF-8", "Europe/Rome"),
        "DE" | "AT" => ("de", "de_DE.UTF-8", "Europe/Berlin"),
        "CH" => ("de", "de_DE.UTF-8", "Europe/Zurich"),
        "FR" | "MC" => ("fr", "fr_FR.UTF-8", "Europe/Paris"),
        "BE" | "LU" => ("fr", "fr_FR.UTF-8", "Europe/Brussels"),
        "ES" => ("es", "es_ES.UTF-8", "Europe/Madrid"),
        "PT" => ("us", "pt_BR.UTF-8", "Europe/Lisbon"),
        "BR" => ("us", "pt_BR.UTF-8", "America/Sao_Paulo"),
        "GB" | "IE" => ("gb", "en_US.UTF-8", "Europe/London"),
        "US" => ("us", "en_US.UTF-8", "America/New_York"),
        "CA" => ("us", "en_US.UTF-8", "America/Toronto"),
        "AU" => ("us", "en_US.UTF-8", "Australia/Sydney"),
        "NZ" => ("us", "en_US.UTF-8", "Pacific/Auckland"),
        "JP" => ("us", "en_US.UTF-8", "Asia/Tokyo"),
        _ => ("us", "en_US.UTF-8", "UTC"),
    };
    let timezone = if validate_timezone(timezone).is_ok() {
        timezone.to_string()
    } else {
        fallback.to_string()
    };
    RegionGuess {
        keymap: keymap.to_string(),
        locale: locale.to_string(),
        timezone,
    }
}

fn json_string(body: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\"");
    let start = body.find(&marker)?;
    let rest = &body[start + marker.len()..];
    let colon = rest.find(':')?;
    let rest = rest[colon + 1..].trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn italy_selects_italian_keyboard_and_language() {
        let guess = from_country("it", "Europe/Rome");
        assert_eq!(guess.keymap, "it");
        assert_eq!(guess.locale, "it_IT.UTF-8");
        assert_eq!(guess.timezone, "Europe/Rome");
    }

    #[test]
    fn geo_json_prefers_the_reported_timezone() {
        let body = r#"{"country_code":"IT","timezone":"Europe/Rome","languages":"it"}"#;
        let guess = guess_from_geo(body).unwrap();
        assert_eq!(guess.keymap, "it");
        assert_eq!(guess.locale, "it_IT.UTF-8");
        assert_eq!(guess.timezone, "Europe/Rome");
    }

    #[test]
    fn unknown_country_stays_on_us_english() {
        let guess = from_country("ZZ", "not a zone");
        assert_eq!(guess.keymap, "us");
        assert_eq!(guess.locale, "en_US.UTF-8");
        assert_eq!(guess.timezone, "UTC");
    }
}
