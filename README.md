Partial Struct
==============

Partial Struct is a procedural macro crate that generates a “partial” version of a Rust struct.
It creates a new struct containing only a subset of the fields from your original struct — omitting those
you specify via the attribute — and automatically provides conversions both ways:

  • The generated struct implements a conversion method named to_<original_struct>() (in snake case)
    that takes values for the omitted fields and reconstructs the full struct.
    
  • Additionally, the full struct implements conversion (via the From trait) into the generated partial struct.
    This lets you write code like: let partial: PartialCar = car.into();

Features
--------
• Generate Partial Structs:
  Automatically create a new struct containing all fields from the original struct except those you omit.

• Custom Target Name:
  Optionally specify the name of the generated struct via a literal. If omitted, the generated struct is named
  "Partial<OriginalStructName>".

• Custom Derives:
  Add extra derives (e.g. Debug, Clone) on the generated struct via a derive(...) clause.

• Bidirectional Conversion:
  The macro implements both a conversion method (to_<original_struct>()) on the generated partial struct and
  an implementation of From<FullStruct> for the partial struct, allowing you to convert back and forth.

Installation
------------
Add the following to your Cargo.toml:

  [dependencies]
  partial_struct = "0.1.0"

Usage
-----
Annotate your struct with #[derive(Partial)] and attach a #[partial(...)] attribute to configure the output.
The attribute supports three optional parts (order does not matter):

  - An optional target name literal (e.g. "UserConstructor"). If omitted, the generated struct is named
    "Partial<OriginalStructName>".
  - An optional derive(...) clause listing trait identifiers to derive on the generated struct.
  - An optional omit(...) clause listing the names of fields to omit from the generated struct.
    
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
  
This generates:
  
  #[derive(Debug, Clone)]
  pub struct UserConstructor {
      pub name: String,
  }
  
  impl UserConstructor {
      pub fn to_user(self, id: uuid::Uuid, secret: String) -> User {
          User { name: self.name, id, secret }
      }
  }
  
  impl From<User> for UserConstructor {
      fn from(full: User) -> Self {
          Self { name: full.name }
      }
  }
  
Example 2: Default Target Name

  #[derive(Partial)]
  #[partial(derive(Debug), omit(x))]
  pub struct Car {
      x: u32,
      model: String,
  }
  
Since no target name is provided, the generated struct is named "PartialCar" and the conversion method is
named "to_car()". Additionally, a From<Car> implementation is provided so that you can convert a Car into a PartialCar.

How It Works
------------
When you derive Partial on a struct, the macro:
  1. Parses the #[partial(...)] attribute to determine:
     - The target name for the generated partial struct.
     - Any additional traits to derive.
     - Which fields to omit.
  2. Generates a new struct containing all fields from the original struct except those specified.
  3. Implements a conversion method on the generated partial struct that takes the omitted fields as parameters and
     returns the full struct.
  4. Implements From<FullStruct> for the generated partial struct, allowing conversion from the full struct
     into the partial version.

Minimizing Build Overhead
-------------------------
This crate minimizes compile time by enabling only the minimal syn features required for parsing.
In the Cargo.toml for this crate, syn is included with:
  
  syn = { version = "2.0.98", default-features = false, features = ["parsing"] }

License
-------
This project is dual-licensed under the MIT or Apache-2.0 license, at your option.

Contributing
------------
Contributions, issues, and feature requests are welcome. Please check the issue tracker on the project's repository
for more information.

Author
------
Esteban <estebanmff@outlook.com>

