use uniprops_gen::UnipropsBuilder;

fn main() {
    UnipropsBuilder::new().build();
    UnipropsBuilder::new()
        .filter(|r| r.general_category == "Nd")
        .out_file("filtered_digits.rs")
        .build();

    UnipropsBuilder::new()
        .with_categories(false)
        .out_file("no_categories.rs")
        .build();

    UnipropsBuilder::new()
        .filter(|r| r.code_point != 0x38)
        .out_file("without_0x38.rs")
        .build();

    UnipropsBuilder::new()
        // 0 and 8
        .filter(|r| [0x30, 0x38].contains(&r.code_point))
        .with_custom(|recs| {
            let array_body: String = recs
                .iter()
                .map(|r| format!("\"{}\",", r.general_category))
                .collect();

            format!(
                "pub const TEST_ARRAY_OF_CATEGORIES: [&'static str; {}] = [{}];",
                recs.len(),
                array_body
            )
        })
        .with_custom(|recs| {
            let array_body: String = recs
                .iter()

                .map(|r| format!("{},", r.decimal_digit_value.unwrap_or(100))/* Unwrap or fail test */)
                .collect();

            format!(
                "pub const TEST_ARRAY_OF_DEC_VALUES: [u8; {}] = [{}];",
                recs.len(),
                array_body
            )
        })
        .with_categories(false)
        .with_digits(false)
        .out_file("custom.rs")
        .build();
}
