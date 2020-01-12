use regex::Regex;

pub fn kebab_case<S: AsRef<str>>(string: S) -> String {
    let string = string.as_ref();

    // Saving memory by compiling the regexes only once throughout the program
    // with the use of the `lazy_static` crate
    lazy_static! {
        static ref WHITESPACE_CHARACTERS: Regex = Regex::new(r"\s+|-+").unwrap();
        static ref INVALID_CHARACTERS: Regex = Regex::new(r"[^A-Za-z0-9]+").unwrap();
    }

    // TODO: Optimize this. This is horrible btw
    // This is the implementation derived from v1
    // There has to be a better way
    let words: Vec<&str> = WHITESPACE_CHARACTERS.split(&string).collect();
    let mut filtered_words: Vec<String> = Vec::new();

    for word in words.iter() {
        if word.is_empty() {
            continue;
        }

        let filtered_word: String = INVALID_CHARACTERS.replace(word, "").to_lowercase();

        if filtered_word.is_empty() {
            continue;
        }

        filtered_words.push(filtered_word);
    }

    filtered_words.join("-")
}

fn capitalize_string<S: AsRef<str>>(string: S) -> Option<String> {
    let string = string.as_ref();

    let mut chars = string.chars();

    if let Some(first_char) = chars.next() {
        let mut resulting_string = first_char.to_uppercase().to_string();
        let string_remainder: String = chars.collect::<String>().to_lowercase();
        resulting_string.push_str(&string_remainder);

        Some(resulting_string)
    } else {
        None
    }
}

pub fn title_case<S: AsRef<str>>(string: S) -> String {
    let string = string.as_ref();
    let mut resulting_string = String::new();

    let mut word_by_whitespace = string.split_whitespace().peekable();
    while let Some(word) = word_by_whitespace.next() {
        let mut capitalized_word = match capitalize_string(word) {
            Some(v) => v,
            None => {
                word_by_whitespace.next();
                continue;
            }
        };

        if word_by_whitespace.peek().is_some() {
            capitalized_word.push(' ');
        }

        resulting_string.push_str(&capitalized_word);
    }

    resulting_string
}

pub fn regex_match(
    regex_str: &str,
    value: &str,
) -> bool {
    let compiled_regex: Regex = Regex::new(regex_str).unwrap();

    compiled_regex.is_match(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! t {
        ($e:ident : $func:expr => $input:expr, $output:expr ) => {{
            let result = $func($input);

            assert_eq!(result, $output);
        }};
    }

    #[test]
    fn kebab_case_test() {
        t!(basic_kebab_case: kebab_case => "The Quick Brown Fox Jumps Over The Lazy Dog.", "the-quick-brown-fox-jumps-over-the-lazy-dog");
        t!(kebab_case_with_hyphens: kebab_case => "The---Quick---Brown Fox Jumps Over---The---Lazy Dog.", "the-quick-brown-fox-jumps-over-the-lazy-dog");
        t!(kebab_case_with_non_alphanumeric_chars: kebab_case => "The Quick Brown Fox: [It Jumps Over The Lazy Dog].", "the-quick-brown-fox-it-jumps-over-the-lazy-dog");
    }

    #[test]
    fn title_case_test() {
        t!(basic_title_case: title_case => "The quick brown fox jumps over the lazy dog.", "The Quick Brown Fox Jumps Over The Lazy Dog.");
        // t!(basic_title_case_with_symbols: title_case => "The quick brown fox jumps [over] [[the]] lazy dog.", "The Quick Brown Fox Jumps [Over] [[The]] Lazy Dog.");
        t!(mixed_title_case: title_case => "ThE qUiCk bRoWn fOx jUmps over tHe laZy dOg.", "The Quick Brown Fox Jumps Over The Lazy Dog.");
    }

    #[test]
    fn basic_regex_match() {
        {
            let test_case = String::from(r"^\w+$");
            let result = regex_match(&test_case.to_string(), "browing");

            assert_eq!(result, true);
        }
    }
}
