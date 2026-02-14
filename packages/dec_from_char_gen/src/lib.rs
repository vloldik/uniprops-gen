use std::io::BufReader;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use serde::Deserialize;

/// Category for digits in UnicodeData.txt
const NUMERIC_CATEGORY: &str = "Nd";

/// Represents a single entry (line) in the UnicodeData.txt file.
///
/// The fields correspond to the specification described in Unicode Standard Annex #44.
/// https://www.unicode.org/reports/tr44/#UnicodeData.txt
#[derive(Debug, Deserialize)]
#[allow(unused)]
struct UnicodeRecord {
    /// Field 0: The character's code point in hexadecimal format.
    pub code_point: String,

    /// Field 1: The official character name. For ranges, this is the name of the starting character.
    pub name: String,

    /// Field 2: The character's general category (e.g., "Lu", "Nd", "So").
    pub general_category: String,

    /// Field 3: The canonical combining class (a number from 0 to 254).
    pub canonical_combining_class: u8,

    /// Field 4: The bidirectional category (e.g., "L", "R", "ON").
    pub bidi_category: String,

    /// Field 5: The character's decomposition. May include a compatibility tag (e.g., "<font>").
    pub decomposition: Option<String>,

    /// Field 6: The numeric value, if the character is a decimal digit.
    pub decimal_digit_value: Option<u32>,

    /// Field 7: The numeric value, if the character is a digit (but not necessarily decimal).
    pub digit_value: Option<u32>,

    /// Field 8: The character's numeric value in a more general sense (including fractions).
    pub numeric_value: Option<String>, // Can be a fraction, so it's a String

    /// Field 9: "Y" (yes) or "N" (no) — whether the character should be mirrored in bidirectional text.
    pub bidi_mirrored: String,

    /// Field 10: The legacy name from Unicode 1.0.
    pub unicode_1_name: Option<String>,

    /// Field 11: The legacy ISO 10646 comment.
    pub iso_comment: Option<String>,

    /// Field 12: Simple uppercase mapping (a single character).
    pub simple_uppercase_mapping: Option<String>,

    /// Field 13: Simple lowercase mapping (a single character).
    pub simple_lowercase_mapping: Option<String>,

    /// Field 14: Simple titlecase mapping (a single character).
    pub simple_titlecase_mapping: Option<String>,
}

#[derive(Debug)]
struct NormalizationReplacement {
    normalized_unicode_char: char,
    ascii_char: u32,
}

fn extract_unicode_char(hex_code_digits: &str) -> char {
    u32::from_str_radix(hex_code_digits, 16)
        .map(char::from_u32)
        .unwrap()
        .expect("Hex MUST be valid unicode")
}

fn parse_digit_mappings() -> Vec<NormalizationReplacement> {
    let file = include_str!("../assets/UnicodeData.txt");
    let reader = BufReader::new(file.as_bytes());
    let mut parser = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(b';')
        .from_reader(reader);

    parser
        .deserialize::<UnicodeRecord>()
        .flatten()
        .filter(|res| res.general_category == NUMERIC_CATEGORY)
        .map(|res| NormalizationReplacement {
            normalized_unicode_char: extract_unicode_char(&res.code_point),
            ascii_char: res
                .decimal_digit_value
                .expect("all \\Nd should have decimal value"),
        })
        .collect()
}

/// This macro creates 'match' structure for each decimal digit with category
/// `\Nd` in unicode `UnicodeData.txt`.
///
/// Since data for parsing is downloaded, saved in git and tested it should always compile.
#[proc_macro]
pub fn digit_parse_mappings(item: TokenStream) -> TokenStream {
    let mappings = parse_digit_mappings()
        .iter()
        .map(|r| {
            let match_char = r.normalized_unicode_char;
            let ascii_digit = r.ascii_char;
            quote! {
                #match_char => ::std::option::Option::Some(#ascii_digit as u8)
            }
        })
        .collect::<Vec<_>>();

    let item = format_ident!("{}", item.to_string());
    quote! {
        match #item {
            #(#mappings,)*
            _ => ::std::option::Option::None
        }
    }
    .into()
}
