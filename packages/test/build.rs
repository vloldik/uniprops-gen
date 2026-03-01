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
}
