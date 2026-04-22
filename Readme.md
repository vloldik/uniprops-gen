# UniProps-gen

[![Crates.io](https://img.shields.io/crates/v/uniprops-gen.svg)](https://crates.io/crates/uniprops-gen)
[![Docs.rs](https://docs.rs/uniprops-gen/badge.svg)](https://docs.rs/uniprops-gen)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://opensource.org/licenses/MIT)

**UniProps** is a blazing-fast, compile-time Unicode property generator for Rust. It generates highly optimized static tables to look up Unicode General Categories, Numeric Values, and custom properties with zero runtime allocation.

> **Note:** This project is a complete evolution and fork of my previous project, `dec-from-char`. While `dec-from-char` focused solely on parsing decimal digits, **UniProps** generalizes this approach. It allows you to generate efficient data structures for *any* subset of Unicode data directly via `build.rs`.

## Todo
* Update metadata actions pipeline

## 🚀 Features

*   **Unmatched Performance with Multiple Strategies:**
    *   **Trie (O(1)):** Uses a Two-Level Trie (Index Table + Data Blocks) for extremely fast, constant-time lookups. Best for dense Unicode data.
    *   **BSearch (O(log N)):** Uses a compact binary search array. Highly recommended when you heavily filter the dataset, allowing the lookup tables to sit comfortably in the CPU's L1 cache for maximum speed.
*   **Zero Runtime Allocation:** All data is baked into your binary as standard `static` arrays.
*   **Customizable & Extensible:** 
    *   Filter by specific categories (e.g., only `Nd` digits) to drastically reduce your binary size.
    *   **Custom Generators:** Inject your own logic to parse and extract specific properties (like bidi categories or case mappings) straight into the generated module.
*   **Safe API:** Generated code relies on safe wrappers around bounded `unsafe` lookups, ensuring maximum speed without bounds-checking overhead, while remaining 100% memory safe.

## ⚡ Benchmarks

| Method | Operation | Time / iter | Notes |
| :--- | :--- | :--- | :--- |
| **Rust Standard Library** | `char::is_numeric()` | **~7.66 ns** | Standard `std` implementation |
| **UniProps Digits** | `get_digit_value(c)` | **~6.15 ns** | **~20% Faster** on mixed text |
| **UniProps Categories** | `Category::from_char(c)` | **~5.22 ns** | **~32% Faster** (O(1) Trie lookup) |

## 📦 Installation

Add `uniprops-gen` to your `[build-dependencies]` in `Cargo.toml`. 

```toml
[build-dependencies]
uniprops-gen = "0.3" # Use the latest version
```

## 🛠 Usage

### 1. Configure `build.rs`

Create a `build.rs` file in your project root. Use the builder to generate your tables into `OUT_DIR`.

```rust
// build.rs
use uniprops_gen::{UnipropsBuilder, LookupStrategy};

fn main() {
    // 1. Generate everything with O(1) Trie lookup (Default)
    UnipropsBuilder::new()
        .out_file("unicode_data.rs")
        .with_categories(true)
        .with_digits(true)
        .build();

    // 2. Generate a specialized BSearch table for a filtered subset
    // BSearch is perfectly suited for sparse data and saves massive amounts of binary size.
    UnipropsBuilder::new()
        .out_file("filtered_data.rs")
        .with_lookup_strategy(LookupStrategy::BSearch)
        .filter(|record| record.general_category == "Nd")
        .build();
        
    // 3. Inject your own custom property generators!
    UnipropsBuilder::new()
        .out_file("custom_data.rs")
        .with_custom(|records| {
            let count = records.len();
            format!("pub const VALID_CODEPOINTS_COUNT: usize = {};", count)
        })
        .build();
}
```

### 2. Include in `lib.rs`

Import the generated code using the `include!` macro.

```rust
// src/lib.rs

pub mod generated {
    // The file name must match what you set in build.rs
    include!(concat!(env!("OUT_DIR"), "/unicode_data.rs"));
}

fn main() {
    use generated::uniprops::{Category, get_digit_value};

    // 1. Fast Category Lookup
    assert_eq!(Category::from_char('A'), Some(Category::Lu)); // Letter, Uppercase
    assert_eq!(Category::from_char('🦀'), Some(Category::So)); // Symbol, Other

    // 2. Fast Digit Value Lookup
    assert_eq!(get_digit_value('9'), Some(9)); // ASCII
    assert_eq!(get_digit_value('٣'), Some(3)); // Arabic-Indic
    assert_eq!(get_digit_value('X'), None);
}
```

## 📄 License

This project is dual-licensed under either of:

*   [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
*   [MIT license](http://opensource.org/licenses/MIT)

at your option.

