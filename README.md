Partial Struct
==============

Partial Struct is a procedural macro crate that generates a “partial” version of a Rust struct.
It creates a new struct containing only a subset of the fields from your original struct—omitting those
you specify via the attribute—and automatically provides a conversion method that reconstructs the full
struct by accepting the omitted fields as parameters.

Features
--------
• Generate Partial Structs:
  Automatically create a new struct that contains all fields from the original struct except those you
  choose to omit.
  
• Custom Target Name:
  Optionally specify the name of the generated struct. If omitted, the default name is "Partial<OriginalStructName>".
  
• Custom Derives:
  Add extra derives (e.g. Debug, Clone) on the generated struct.
  
• Conversion Method:
  The macro implements a conversion method named `to_<original_struct>()` (with the original struct’s name in snake case)
  on the generated struct. This method accepts values for the omitted fields and returns an instance of the full struct.

Installation
------------
Add the following to your Cargo.toml:

  [dependencies]
  partial_struct = "0.1.0"

Usage
-----
Annotate your struct with `#[derive(Partial)]` and attach the `#[partial(...)]` attribute to customize the output.
The attribute supports three optional parts (order does not matter):

  - An optional target name literal, e.g. "UserConstructor". If omitted, the generated struct will be named
    "Partial<OriginalStructName>".
  - An optional `derive(...)` clause listing trait identifiers to derive on the generated struct.
  - An optional `omit(...)` clause listing the names of fields (as idents) to omit from the generated struct.
    
Examples
--------

Example 1: Explicit Target Name, Extra Derives, and Omitted Fields

  #[derive(Partial)]
  #[partial("UserConstructor", derive(Debug, Clone), omit(id, secret))]
  pub struct User {
      id: uuid::Uuid,
      name: String,
      secret: String,
  }
  
The macro generates:

  #[derive(Debug, Clone)]
  pub struct UserConstructor {
      pub name: String,
  }
  
  impl UserConstructor {
      pub fn to_user(self, id: uuid::Uuid, secret: String) -> User {
          User { name: self.name, id, secret }
      }
  }
  
Example 2: Using Default Target Name

  #[derive(Partial)]
  #[partial(derive(Debug), omit(x))]
  pub struct Car {
      x: u32,
      model: String,
  }
  
Since no target name is provided, the generated struct is named "PartialCar" and the conversion method is
named `to_car()`.

How It Works
------------
When you derive `Partial` on a struct, the macro:
  1. Parses the `#[partial(...)]` attribute to determine:
     - The target name for the generated struct.
     - Any additional traits to derive on the generated struct.
     - Which fields to omit.
  2. Generates a new struct containing all fields from the original struct except those listed in the
     `omit(...)` clause.
  3. Implements a conversion method named `to_<original_struct>()` (using snake case) that accepts values
     for the omitted fields and reconstructs the full struct.

Minimizing Build Overhead
-------------------------
This crate minimizes compile time by enabling only the minimal features required for parsing.
In your proc-macro crate’s Cargo.toml, use:

  [dependencies]
  heck = "0.5.0"
  proc-macro2 = "1.0.93"
  quote = "1.0.38"
  syn = { version = "2.0.98", default-features = false, features = ["parsing"] }

License
-------
This project is dual-licensed under the MIT or Apache-2.0 license, at your option.

Contributing
------------
Contributions, issues, and feature requests are welcome. Please check the issue tracker on the project’s repository for more information.

Author
------
Esteban <estebanmff@outlook.com>

