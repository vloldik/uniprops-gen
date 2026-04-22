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

/// Defines the internal data structure and algorithm used for character property lookups.
///
/// This enum allows you to explicitly control the trade-off between binary size,
/// memory access patterns, and runtime lookup performance based on the shape of your data.
///
/// # Mini-Guide: Trie vs BSearch
///
/// 1. **Default Unicode Data (Dense)**: Use `Trie { shift: 8 }`. It provides `O(1)` performance
///    and the array size is acceptable (~30-40KB for typical categories).
/// 2. **Heavily Filtered Data (Sparse)**: If your `.filter()` closure discards ~90-99% of
///    codepoints (e.g., keeping only identifiers, or a specific script), **use `BSearch`**.
///    The resulting array will be tiny, and the binary search will comfortably sit in the L1 cache,
///    often outperforming `Trie` by avoiding dependent memory fetches.
#[derive(Debug, Clone, Copy)]
pub enum LookupStrategy {
    /// Generates a sorted array of contiguous codepoint ranges and performs a binary search (`O(log N)`).
    ///
    /// **Performance Characteristics:**
    /// While mathematically `O(log N)` is slower than `O(1)`, `BSearch` can actually be **faster**
    /// than a `Trie` in sparse datasets. If you heavily filter the `UnicodeRecord`s, the number of
    /// contiguous ranges (`N`) drops significantly.
    ///
    /// If `N` is small (e.g., under 30-50 ranges), the entire array fits into a single or a few
    /// CPU cache lines (L1 cache). A binary search over hot L1 cache is extremely fast and avoids
    /// the pointer-chasing (dependent loads) inherent to the `Trie` strategy.
    ///
    /// **When to use:**
    /// - You apply strict filters (e.g., keeping only ASCII + specific Unicode blocks).
    /// - You are targeting memory-constrained environments (Wasm, embedded).
    BSearch,

    /// Generates a two-level pre-computed array (Trie) for `O(1)` constant-time lookups.
    ///
    /// This strategy chunks the codepoint space into blocks of size `2^shift`. It generates an
    /// `INDICES` array pointing to deduplicated `BLOCKS`.
    ///
    /// **Performance Characteristics:**
    /// Lookups require exactly two memory reads: one from the `INDICES` array, and a dependent read
    /// from the `BLOCKS` array. For large, dense datasets (like the full Unicode categories list),
    /// this reliably beats binary search. However, if the data is not in the CPU cache, these
    /// two dependent loads can cause cache misses.
    ///
    /// **Choosing the `shift` value:**
    /// The `shift` dictates the block size (`2^shift`).
    /// - **`shift = 8` (256 codepoints per block)**: The universally recommended default. It perfectly
    ///   balances the size of the index array and the efficiency of block deduplication.
    /// - **Smaller shift (e.g., `4` -> 16 codepoints)**: Generates many small blocks. Deduplication
    ///   is highly precise, but the `INDICES` array becomes massive (up to 69,632 elements).
    /// - **Larger shift (e.g., `12` -> 4096 codepoints)**: `INDICES` array is tiny, but deduplication
    ///   suffers. A single valid codepoint in a block forces the allocation of 4,095 empty slots
    ///   (unless an identical block already exists).
    Trie { shift: u8 },
}

#[derive(Debug, Clone)]
struct MappingGroup {
    general_category: String,
    start: u32,
    end: u32,
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

type CustomGenerator<'a> = Box<dyn Fn(&[UnicodeRecord]) -> String + 'a>;

/// A builder for generating a Rust source file containing Unicode property tables and lookups.
///
/// This builder processes the `UnicodeData.txt` asset and generates optimized lookup
/// tables for character categories and decimal digit values.
pub struct UnipropsBuilder<'a> {
    out_name: String,
    gen_categories: bool,
    gen_digits: bool,
    lookup_strategy: LookupStrategy,
    filter: Box<dyn Fn(&UnicodeRecord) -> bool + 'a>,
    custom_generators: Vec<CustomGenerator<'a>>,
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
            lookup_strategy: LookupStrategy::Trie { shift: 8 },
            filter: Box::new(|_| true),
            custom_generators: Default::default(),
        }
    }

    /// Sets the name of the generated output file.
    ///
    /// The file will be created in the directory specified by the `OUT_DIR` environment variable.
    pub fn out_file(mut self, name: &str) -> Self {
        self.out_name = name.to_string();
        self
    }

    /// Overrides the internal lookup strategy for the generated category tables.
    ///
    /// By default, the builder uses `LookupStrategy::Trie { shift: 8 }` which is optimized for
    /// the full Unicode dataset.
    ///
    /// You should override this to `LookupStrategy::BSearch` if you are applying an aggressive
    /// `.filter()` closure that discards the majority of codepoints. In such scenarios, `BSearch`
    /// dramatically shrinks the compiled binary size and often executes faster due to L1 cache locality.
    pub fn with_lookup_strategy(mut self, lookup_strategy: LookupStrategy) -> Self {
        self.lookup_strategy = lookup_strategy;
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

    /// Registers a custom code generator function.
    ///
    /// Allows injecting custom properties extracted from `UnicodeData.txt` into the generated module
    /// without modifying the core builder.
    ///
    /// The closure receives a slice of all parsed and **filtered** `UnicodeRecord`s. It must return
    /// a `String` containing valid Rust code (e.g., custom `static` arrays or `const`s). This string
    /// is parsed into a `TokenStream` and embedded directly into the final `uniprops` module.
    ///
    /// # Example
    /// ```ignore
    /// builder.with_custom(|records| {
    ///     let count = records.len();
    ///     format!("pub const VALID_CODEPOINTS_COUNT: usize = {};", count)
    /// })
    /// ```
    pub fn with_custom<F>(mut self, f: F) -> Self
    where
        F: Fn(&[UnicodeRecord]) -> String + 'a,
    {
        self.custom_generators.push(Box::new(f));
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
            match self.lookup_strategy {
                LookupStrategy::BSearch => self.generate_bsearch_impl(&raw_data),
                LookupStrategy::Trie { shift } => self.generate_trie_impl(shift, &raw_data),
            }
        } else {
            quote! {}
        };

        let digits = if self.gen_digits {
            self.generate_digits(&raw_data)
        } else {
            quote! {}
        };

        let mut custom_tokens = proc_macro2::TokenStream::new();

        for generator in self.custom_generators {
            let generated_str = generator(&raw_data);
            let parsed: TokenStream = generated_str
                .parse()
                .expect("Custom generator returned invalid Rust-code");

            custom_tokens.extend(parsed);
        }

        let tokens = quote! {
            #[allow(clippy::all)]
            #[allow(dead_code)]
            #[allow(non_upper_case_globals)]
            #[rustfmt::skip]
            pub mod uniprops {
                #categories
                #digits
                #custom_tokens
            }
        };

        let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set by cargo");
        let dest_path = Path::new(&out_dir).join(&self.out_name);

        let mut file = File::create(&dest_path).expect("Failed to create output file");
        file.write_all(tokens.to_string().as_bytes())
            .expect("Failed to write to output file");
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

    fn get_mapping_groups(raw_data: &[UnicodeRecord]) -> Vec<MappingGroup> {
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

        mapping_groups
    }

    fn get_unique_categories_sorted(mapping_groups: &[MappingGroup]) -> Vec<String> {
        let mut categories = mapping_groups
            .iter()
            .map(|g| g.general_category.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        categories.sort();
        categories
    }

    fn generate_category_enum(unique_categories: &[String]) -> proc_macro2::TokenStream {
        let enum_variants = unique_categories.iter().map(|cat| {
            let ident = format_ident!("{}", cat);
            quote! { #ident }
        });

        quote! {
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
            pub enum Category {
                #(#enum_variants),*
            }
        }
    }

    fn generate_bsearch_impl(&self, raw_data: &[UnicodeRecord]) -> TokenStream {
        let mut mapping_groups = Self::get_mapping_groups(raw_data);
        let unique_categories = Self::get_unique_categories_sorted(&mapping_groups);
        let category_enum = Self::generate_category_enum(&unique_categories);

        mapping_groups.sort_by(|a, b| a.start.cmp(&b.start));

        let mapping_group_lookup = mapping_groups
            .into_iter()
            .map(|group| {
                let enum_variant = format_ident!("{}", group.general_category);
                let (start, end) = (group.start, group.end);

                quote! {
                    CategoryBounds { start: #start, end: #end, category: Category::#enum_variant }
                }
            })
            .collect::<Vec<_>>();

        let len = mapping_group_lookup.len();

        quote! {
            #category_enum

            struct CategoryBounds {
                start: u32,
                end: u32,
                category: Category,
            }

            static CATEGORY_LOOKUP: [CategoryBounds; #len] = [
                #(#mapping_group_lookup),*
            ];

            impl Category {
                #[inline(always)]
                pub fn from_char(c: char) -> ::std::option::Option<Self> {
                    CATEGORY_LOOKUP.binary_search_by(| g | {
                        let code_point = c as u32;

                        if code_point < g.start {
                            ::core::cmp::Ordering::Greater
                        } else if code_point > g.end {
                            ::core::cmp::Ordering::Less
                        } else {
                            ::core::cmp::Ordering::Equal
                        }
                    })
                    .ok()
                    .map(| i |
                        // SAFETY: We found an element with index i just now, it MUST be in array
                        unsafe { CATEGORY_LOOKUP.get_unchecked(i) }.category
                    )
                }
            }
        }
    }

    fn generate_trie_impl(&self, shift: u8, raw_data: &[UnicodeRecord]) -> TokenStream {
        let size: u32 = 1 << (shift as u32);
        let mask: u32 = size - 1;
        let mapping_groups = Self::get_mapping_groups(raw_data);
        let unique_categories = Self::get_unique_categories_sorted(&mapping_groups);
        let category_enum = Self::generate_category_enum(&unique_categories);

        let max_codepoint: u32 = 0x10FFFF;
        let mut unique_blocks: Vec<Vec<Option<String>>> = Vec::new();
        let mut indices: Vec<usize> = Vec::new();
        let mut group_iter = mapping_groups.iter();
        let mut current_group = group_iter.next();

        for chunk_start in (0..=max_codepoint).step_by(size as usize) {
            let mut block = Vec::with_capacity(size as usize);

            for i in 0..size {
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

        let index_type = if unique_blocks.len() <= (u8::MAX as usize) + 1 {
            quote! { u8 }
        } else if unique_blocks.len() <= (u16::MAX as usize) + 1 {
            quote! { u16 }
        } else {
            quote! { compile_error!("Shift is too small, u16 overflow") }
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
        let blocks_len = unique_blocks.len() * (size as usize);

        quote! {
            #category_enum

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

                    let index_idx = (cp >> #shift) as usize;

                    // SAFETY: Arrays are generated to cover up to 0x10FFFF
                    unsafe {
                        let block_idx = *CATEGORY_INDICES.get_unchecked(index_idx) as usize;
                        let offset = (cp & #mask) as usize;
                        let final_pos = (block_idx << #shift) + offset;
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

        let has_all_ascii_digits =
            (0x30..=0x39).all(|cp| raw_data.binary_search_by_key(&cp, |r| r.code_point).is_ok());

        // Generate fast path only if there are all of the digits presented in raw_data
        let fast_path = if has_all_ascii_digits {
            quote! {
            if cp <= 0x7F {
                return if cp >= 0x30 && cp <= 0x39 { // '0'..='9'
                    ::std::option::Option::Some((cp - 0x30) as u8)
                } else {
                    ::std::option::Option::None
                };
            }}
        } else {
            quote! {}
        };

        quote! {
            static DIGIT_STARTS: [u32; #len] = [ #(#starts),* ];
            static DIGIT_ENDS:   [u32; #len] = [ #(#ends),*   ];
            static DIGIT_BASES:  [u8;  #len] = [ #(#bases),*  ];

            #[inline(always)]
            pub fn get_digit_value(c: char) -> ::std::option::Option<u8> {
                let cp = c as u32;

                // Fast path for ascii
                #fast_path

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
