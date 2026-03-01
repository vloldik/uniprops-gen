pub mod generated_uniprops {
    include!(concat!(env!("OUT_DIR"), "/generated_uniprops.rs"));
}
pub mod filtered_digits {
    include!(concat!(env!("OUT_DIR"), "/filtered_digits.rs"));
}

pub mod no_categories {
    include!(concat!(env!("OUT_DIR"), "/no_categories.rs"));
}

pub mod without_0x38 {
    include!(concat!(env!("OUT_DIR"), "/without_0x38.rs"));
}

#[cfg(test)]
mod tests {
    use std::char;

    use super::*;

    #[test]
    fn test_ascii_categories() {
        use generated_uniprops::uniprops::Category;

        assert_eq!(Category::from_char('A'), Some(Category::Lu)); // Letter, Uppercase
        assert_eq!(Category::from_char('a'), Some(Category::Ll)); // Letter, Lowercase
        assert_eq!(Category::from_char('0'), Some(Category::Nd)); // Number, Decimal Digit
        assert_eq!(Category::from_char(' '), Some(Category::Zs)); // Separator, Space
        assert_eq!(Category::from_char('.'), Some(Category::Po)); // Punctuation, Other
        assert_eq!(Category::from_char('\n'), Some(Category::Cc)); // Other, Control
    }

    #[test]
    fn test_unicode_categories() {
        use generated_uniprops::uniprops::Category;

        assert_eq!(Category::from_char('Д'), Some(Category::Lu));
        assert_eq!(Category::from_char('д'), Some(Category::Ll));

        assert_eq!(Category::from_char('😊'), Some(Category::So));

        assert_eq!(Category::from_char('∑'), Some(Category::Sm));

        assert_eq!(Category::from_char('\u{200B}'), Some(Category::Cf));
    }

    #[test]
    fn test_cjk_ideographs_ranges() {
        use generated_uniprops::uniprops::Category;

        assert_eq!(Category::from_char('\u{4E00}'), Some(Category::Lo)); // First
        assert_eq!(Category::from_char('\u{5B57}'), Some(Category::Lo)); // Middle ('字')
        assert_eq!(Category::from_char('\u{9FFF}'), Some(Category::Lo)); // Last
    }

    #[test]
    fn test_ascii_digits_fast_path() {
        use generated_uniprops::uniprops::get_digit_value;

        for i in 0..=9 {
            let c = char::from_u32('0' as u32 + i).unwrap();
            assert_eq!(get_digit_value(c), Some(i as u8));
        }

        assert_eq!(get_digit_value('/'), None);
        assert_eq!(get_digit_value(':'), None);
        assert_eq!(get_digit_value('a'), None);
    }

    #[test]
    fn test_unicode_digits_binary_search() {
        use generated_uniprops::uniprops::get_digit_value;

        for i in 0..=9 {
            let c = char::from_u32(0xFF10 + i).unwrap();
            assert_eq!(get_digit_value(c), Some(i as u8));
        }

        for i in 0..=9 {
            let c = char::from_u32(0x1D7CE + i).unwrap();
            assert_eq!(get_digit_value(c), Some(i as u8));
        }

        for i in 0..=9 {
            let c = char::from_u32(0x0660 + i).unwrap();
            assert_eq!(get_digit_value(c), Some(i as u8));
        }
        assert_eq!(get_digit_value(char::from_u32(0xFF0F).unwrap()), None); // Before fullwidth 0
        assert_eq!(get_digit_value(char::from_u32(0xFF1A).unwrap()), None); // After fullwidth 9
    }

    #[test]
    fn test_exhaustively_all_unicode_chars() {
        use generated_uniprops::uniprops;

        for cp in 0..=0x10FFFF {
            if let Some(c) = char::from_u32(cp) {
                let cat = uniprops::Category::from_char(c);
                let dig = uniprops::get_digit_value(c);
                if dig.is_some() {
                    assert_eq!(
                        cat,
                        Some(uniprops::Category::Nd),
                        "U+{:04X} ({}): Has digit but not Nd category!",
                        cp,
                        c
                    );
                }
                if cat == Some(uniprops::Category::Nd) {
                    assert!(
                        dig.is_some(),
                        "U+{:04X} ({}): Has category Nd, but no digit value returned!",
                        cp,
                        c
                    );
                }
            }
        }
    }

    #[test]
    fn test_filtered_digits_module() {
        use filtered_digits::uniprops::{Category, get_digit_value};

        assert_eq!(Category::from_char('0'), Some(Category::Nd));
        assert_eq!(Category::from_char('7'), Some(Category::Nd));

        assert_eq!(Category::from_char('A'), None);
        assert_eq!(Category::from_char('a'), None);
        assert_eq!(Category::from_char(' '), None);

        assert_eq!(get_digit_value('5'), Some(5));
        assert_eq!(get_digit_value('X'), None);
    }

    #[test]
    fn test_no_categories_module() {
        use filtered_digits::uniprops::get_digit_value;

        assert_eq!(get_digit_value('9'), Some(9));
        assert_eq!(get_digit_value('Z'), None);
    }
    #[test]
    fn test_exhaustive_no_panic_ub() {
        use generated_uniprops::uniprops::{Category, get_digit_value};

        for cp in 0..=0x10FFFF {
            if let Some(c) = char::from_u32(cp) {
                let _cat = Category::from_char(c);
                let _dig = get_digit_value(c);
            }
        }
    }

    #[test]
    fn test_extreme_codepoints() {
        use generated_uniprops::uniprops::Category;

        assert_eq!(Category::from_char('\u{0000}'), Some(Category::Cc));

        assert_eq!(Category::from_char('\u{10FFFF}'), None);

        let _ = Category::from_char('\u{10FFFD}');
    }

    #[test]
    fn test_trie_chunk_boundaries() {
        use generated_uniprops::uniprops::Category;

        let boundaries = [
            (0x00FF, 0x0100),
            (0x01FF, 0x0200),
            (0x0FFF, 0x1000),  // 4095 / 4096
            (0xFFFF, 0x10000), //
        ];

        for (end_of_block, start_of_next) in boundaries {
            if let Some(c1) = char::from_u32(end_of_block) {
                let _ = Category::from_char(c1);
            }
            if let Some(c2) = char::from_u32(start_of_next) {
                let _ = Category::from_char(c2);
            }
        }
    }

    #[test]
    fn test_surrogate_neighbors() {
        use generated_uniprops::uniprops::Category;

        assert_eq!(Category::from_char('\u{D7FF}'), None); // Unassigned
        assert_eq!(Category::from_char('\u{D7A3}'), Some(Category::Lo));
        assert_eq!(Category::from_char('\u{E000}'), Some(Category::Co));
    }

    #[test]
    fn test_digit_array_bounds() {
        use generated_uniprops::uniprops::get_digit_value;

        assert_eq!(get_digit_value('\u{0030}'), Some(0));

        assert_eq!(get_digit_value('\u{002F}'), None); // '/'

        assert_eq!(get_digit_value('\u{1D7E1}'), Some(9));

        assert_eq!(get_digit_value('\u{1D7E2}'), Some(0));

        assert_eq!(get_digit_value('\u{1D800}'), None);
    }

    #[test]
    fn test_if_excluded_digit_not_exists() {
        use without_0x38::uniprops::{Category, get_digit_value};

        assert_eq!(get_digit_value('\u{38}'), None);
        assert_eq!(get_digit_value('\u{37}'), Some(7));
        assert_eq!(get_digit_value('\u{39}'), Some(9));

        assert_eq!(Category::from_char('\u{38}'), None);
        assert_eq!(Category::from_char('\u{37}'), Some(Category::Nd));
        assert_eq!(Category::from_char('\u{39}'), Some(Category::Nd));
    }
}
