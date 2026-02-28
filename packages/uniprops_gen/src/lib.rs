use proc_macro2::TokenStream;
// build.rs
use quote::{format_ident, quote};
use serde::{Deserialize, Deserializer};
use std::{
    collections::HashSet,
    env,
    fs::File,
    io::{BufReader, Write},
    path::Path,
    process::Command,
};

fn deserialize_hex_u32<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    u32::from_str_radix(&s, 16).map_err(serde::de::Error::custom)
}

#[derive(Debug, Deserialize, Clone)]
#[allow(unused)]
pub struct UnicodeRecord {
    #[serde(deserialize_with = "deserialize_hex_u32")]
    pub code_point: u32,
    pub name: String,
    pub general_category: String,
    pub canonical_combining_class: u8,
    pub bidi_category: String,
    pub decomposition: Option<String>,
    pub decimal_digit_value: Option<u32>,
    pub digit_value: Option<u32>,
    pub numeric_value: Option<String>,
    pub bidi_mirrored: String,
    pub unicode_1_name: Option<String>,
    pub iso_comment: Option<String>,
    pub simple_uppercase_mapping: Option<String>,
    pub simple_lowercase_mapping: Option<String>,
    pub simple_titlecase_mapping: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PositionTag {
    First,
    Last,
    None,
}

fn get_tag_by_name(name: &str) -> PositionTag {
    let Some(delim) = name.split(',').nth(1) else {
        return PositionTag::None;
    };
    let tag = delim.trim_matches(|c| c == ' ' || c == '>');
    if tag == "First" {
        PositionTag::First
    } else {
        PositionTag::Last
    }
}

/// A builder for generating a Rust source file containing Unicode property tables and lookups.
///
/// This builder processes the `UnicodeData.txt` asset and generates optimized lookup
/// tables for character categories and decimal digit values.
pub struct UnipropsBuilder<'a> {
    out_name: String,
    gen_categories: bool,
    gen_digits: bool,
    filter: Box<dyn Fn(&UnicodeRecord) -> bool + 'a>,
}

impl<'a> UnipropsBuilder<'a> {
    /// Creates a new `UnipropsBuilder` with default settings.
    ///
    /// By default, it generates `generated_uniprops.rs` with both categories
    /// and digit values enabled.
    pub fn new() -> Self {
        Self {
            out_name: "generated_uniprops.rs".to_string(),
            gen_categories: true,
            gen_digits: true,
            filter: Box::new(|_| true),
        }
    }

    /// Sets the name of the generated output file.
    ///
    /// The file will be created in the directory specified by the `OUT_DIR` environment variable.
    pub fn out_file(mut self, name: &str) -> Self {
        self.out_name = name.to_string();
        self
    }

    /// Toggles the generation of the `Category` enum and character-to-category mapping.
    pub fn with_categories(mut self, enable: bool) -> Self {
        self.gen_categories = enable;
        self
    }

    /// Toggles the generation of the `get_digit_value` function and its associated tables.
    pub fn with_digits(mut self, enable: bool) -> Self {
        self.gen_digits = enable;
        self
    }

    /// Sets a filter to include only specific Unicode records in the generation process.
    ///
    /// Records that return `false` from the filter will be ignored.
    pub fn filter<F>(mut self, filter: F) -> Self
    where
        F: Fn(&UnicodeRecord) -> bool + 'a,
    {
        self.filter = Box::new(filter);
        self
    }

    /// Executes the code generation process.
    ///
    /// This method:
    /// 1. Parses the Unicode data.
    /// 2. Generates optimized lookup tables (multi-level arrays for categories and ranges for digits).
    /// 3. Writes the resulting Rust code to the output file.
    /// 4. Attempts to format the generated file using `rustfmt`.
    ///
    /// # Panics
    /// Panics if `OUT_DIR` is not set, or if there are errors reading the asset or writing the output file.
    pub fn build(self) {
        let raw_data = self.parse_data();

        let categories = if self.gen_categories {
            self.generate_categories(&raw_data)
        } else {
            quote! {}
        };

        let digits = if self.gen_digits {
            self.generate_digits(&raw_data)
        } else {
            quote! {}
        };

        let tokens = quote! {
            #[allow(clippy::all)]
            #[allow(dead_code)]
            #[allow(non_upper_case_globals)]
            pub mod uniprops {
                #categories
                #digits
            }
        };

        let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set by cargo");
        let dest_path = Path::new(&out_dir).join(&self.out_name);

        let mut file = File::create(&dest_path).expect("Failed to create output file");
        file.write_all(tokens.to_string().as_bytes())
            .expect("Failed to write to output file");

        let _ = Command::new("rustfmt").arg(&dest_path).status();
    }

    fn parse_data(&self) -> Vec<UnicodeRecord> {
        let reader = BufReader::new(include_str!("../assets/UnicodeData.txt").as_bytes());
        let mut parser = csv::ReaderBuilder::new()
            .has_headers(false)
            .delimiter(b';')
            .from_reader(reader);

        let mut raw_data = Vec::new();
        for result in parser.deserialize::<UnicodeRecord>() {
            let record = result.expect("CSV Parse Error");
            if (self.filter)(&record) {
                raw_data.push(record);
            }
        }
        raw_data.sort_by_key(|r| r.code_point);
        raw_data
    }

    fn generate_categories(&self, raw_data: &[UnicodeRecord]) -> TokenStream {
        const SHIFT: u32 = 8;
        const SIZE: u32 = 1 << SHIFT;
        const MASK: u32 = SIZE - 1;

        struct MappingGroup {
            general_category: String,
            start: u32,
            end: u32,
        }

        let mut mapping_groups = Vec::new();

        if !raw_data.is_empty() {
            let record = &raw_data[0];
            let mut current_group = MappingGroup {
                general_category: record.general_category.clone(),
                start: record.code_point,
                end: record.code_point,
            };

            for record in raw_data.iter().skip(1) {
                let was_groupped = get_tag_by_name(&record.name) == PositionTag::Last;
                if (record.code_point == current_group.end + 1
                    && record.general_category == current_group.general_category)
                    || was_groupped
                {
                    current_group.end = record.code_point;
                } else {
                    mapping_groups.push(current_group);
                    current_group = MappingGroup {
                        general_category: record.general_category.clone(),
                        start: record.code_point,
                        end: record.code_point,
                    };
                }
            }
            mapping_groups.push(current_group);
        }

        let mut unique_categories: Vec<String> = mapping_groups
            .iter()
            .map(|g| g.general_category.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        unique_categories.sort();

        let enum_variants = unique_categories.iter().map(|cat| {
            let ident = format_ident!("{}", cat);
            quote! { #ident }
        });

        let max_codepoint: u32 = 0x10FFFF;
        let mut unique_blocks: Vec<Vec<Option<String>>> = Vec::new();
        let mut indices: Vec<usize> = Vec::new();
        let mut group_iter = mapping_groups.iter();
        let mut current_group = group_iter.next();

        for chunk_start in (0..=max_codepoint).step_by(SIZE as usize) {
            let mut block = Vec::with_capacity(SIZE as usize);

            for i in 0..SIZE {
                let cp = chunk_start + i;
                while let Some(g) = current_group {
                    if cp > g.end {
                        current_group = group_iter.next();
                    } else {
                        break;
                    }
                }

                let category = if let Some(g) = current_group {
                    if cp >= g.start && cp <= g.end {
                        Some(g.general_category.clone())
                    } else {
                        None
                    }
                } else {
                    None
                };
                block.push(category);
            }

            if let Some(idx) = unique_blocks.iter().position(|b| b == &block) {
                indices.push(idx);
            } else {
                indices.push(unique_blocks.len());
                unique_blocks.push(block);
            }
        }

        let index_type = if unique_blocks.len() <= 256 {
            quote! { u8 }
        } else {
            quote! { u16 }
        };
        let indices_tokens = indices.iter().map(|&idx| {
            if unique_blocks.len() <= 256 {
                let val = idx as u8;
                quote! { #val }
            } else {
                let val = idx as u16;
                quote! { #val }
            }
        });

        let indices_len = indices.len();
        let blocks_tokens = unique_blocks.iter().flatten().map(|opt_cat| match opt_cat {
            Some(cat) => {
                let ident = format_ident!("{}", cat);
                quote! { Some(Category::#ident) }
            }
            None => quote! { None },
        });
        let blocks_len = unique_blocks.len() * (SIZE as usize);

        quote! {
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
            pub enum Category {
                #(#enum_variants),*
            }

            static CATEGORY_INDICES:[#index_type; #indices_len] = [
                #(#indices_tokens),*
            ];

            static CATEGORY_BLOCKS: [Option<Category>; #blocks_len] =[
                #(#blocks_tokens),*
            ];

            impl Category {
                #[inline(always)]
                pub fn from_char(c: char) -> ::std::option::Option<Self> {
                    let cp = c as u32;
                    if cp > #max_codepoint { return None; }

                    let index_idx = (cp >> #SHIFT) as usize;

                    // SAFETY: Arrays are generated to cover up to 0x10FFFF
                    unsafe {
                        let block_idx = *CATEGORY_INDICES.get_unchecked(index_idx) as usize;
                        let offset = (cp & #MASK) as usize;
                        let final_pos = (block_idx << #SHIFT) + offset;
                        *CATEGORY_BLOCKS.get_unchecked(final_pos)
                    }
                }
            }
        }
    }

    fn generate_digits(&self, raw_data: &[UnicodeRecord]) -> TokenStream {
        struct DigitRange {
            start: u32,
            end: u32,
            base_val: u8,
        }

        let mut ranges: Vec<DigitRange> = Vec::new();

        for r in raw_data {
            let Some(dig_val) = r.decimal_digit_value else {
                continue;
            };
            let dig_val = dig_val as u8;

            if let Some(last) = ranges.last_mut() {
                let is_contiguous_cp = r.code_point == last.end + 1;
                let expected_val = last.base_val as u32 + (r.code_point - last.start);

                if is_contiguous_cp && dig_val as u32 == expected_val {
                    last.end = r.code_point;
                    continue;
                }
            }
            ranges.push(DigitRange {
                start: r.code_point,
                end: r.code_point,
                base_val: dig_val,
            });
        }

        let starts: Vec<u32> = ranges.iter().map(|r| r.start).collect();
        let ends: Vec<u32> = ranges.iter().map(|r| r.end).collect();
        let bases: Vec<u8> = ranges.iter().map(|r| r.base_val).collect();
        let len = ranges.len();

        quote! {
            static DIGIT_STARTS: [u32; #len] = [ #(#starts),* ];
            static DIGIT_ENDS:   [u32; #len] = [ #(#ends),*   ];
            static DIGIT_BASES:  [u8;  #len] = [ #(#bases),*  ];

            #[inline(always)]
            pub fn get_digit_value(c: char) -> ::std::option::Option<u8> {
                let cp = c as u32;

                // Fast path for ascii
                if cp <= 0x7F {
                    return if cp >= 0x30 && cp <= 0x39 { // '0'..='9'
                        ::std::option::Option::Some((cp - 0x30) as u8)
                    } else {
                        ::std::option::Option::None
                    };
                }

                let idx = DIGIT_STARTS.partition_point(|&start| start <= cp);

                if idx > 0 {
                    let i = idx - 1;
                    if cp <= DIGIT_ENDS[i] {
                        let offset = cp - DIGIT_STARTS[i];
                        return ::std::option::Option::Some(DIGIT_BASES[i] + offset as u8);
                    }
                }
                ::std::option::Option::None
            }
        }
    }
}

impl<'a> Default for UnipropsBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}
