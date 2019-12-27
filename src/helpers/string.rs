use regex::Regex;

pub fn kebab_case(string: &str) -> String {
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

    #[test]
    fn title_case_to_kebab_case() {
        {
            let test_case = String::from("The Quick Brown Fox Jumps Over The Lazy Dog.");
            let result = kebab_case(&test_case);

            assert_eq!(result, "the-quick-brown-fox-jumps-over-the-lazy-dog");
        };

        {
            let test_case = String::from("The---Quick---Brown Fox Jumps Over---The---Lazy Dog.");
            let result = kebab_case(&test_case);

            assert_eq!(result, "the-quick-brown-fox-jumps-over-the-lazy-dog");
        };

        {
            let test_case = String::from("The Quick Brown Fox: [It Jumps Over The Lazy Dog].");
            let result = kebab_case(&test_case);

            assert_eq!(result, "the-quick-brown-fox-it-jumps-over-the-lazy-dog");
        };
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
