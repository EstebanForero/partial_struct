Partial Struct
==============

Partial Struct is a procedural macro crate that generates “partial” versions of Rust structs.
It creates new structs that contain a subset of the fields from your original struct—omitting those you specify
via the attribute—and automatically provides bidirectional conversion functions.

Features
--------
• Generate Partial Structs:
  Automatically create one or more partial structs for a given full struct. Each partial struct contains all
  fields from the full struct except those specified to omit.

• Custom Target Name:
  Optionally specify the name of the generated partial struct via a literal. If omitted, the generated struct is
  named "Partial<OriginalStructName>".

• Custom Derives:
  Optionally add extra derives (e.g., Debug, Clone) to the generated partial struct using a derive(...) clause.

• Bidirectional Conversion:
  The macro implements two conversions:
    - A method on the generated partial struct (named to_<base_struct>() in snake case) that takes
      the omitted fields as parameters and reconstructs the full struct.
    - An implementation of From<FullStruct> for the generated partial struct, so you can convert the full struct
      into its partial representation via .into().

Installation
------------
Add the following to your Cargo.toml:

  [dependencies]
  partial_struct = "0.1.0"

Usage
-----
Annotate your struct with #[derive(Partial)] and attach one or more #[partial(...)] attributes to configure the output.
The attribute supports three optional parts (order does not matter):

  - An optional target name literal (e.g. "UserConstructor"). If omitted, the generated struct is named
    "Partial<OriginalStructName>".
  - An optional derive(...) clause listing trait identifiers to derive on the generated struct.
  - An optional omit(...) clause listing the names of fields to omit from the generated struct.

Examples
--------

Example 1: Explicit Target Name, Extra Derives, and Omitted Fields

```
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
```
  
  
Example 2: Default Target Name

```
#[derive(Partial)]
  #[partial(derive(Debug), omit(x))]
  pub struct Car {
      x: u32,
      model: String,
  }
```
  
  
Since no target name is provided, the generated struct is named "PartialCar" and the conversion method is
named "to_car()". Also, an implementation of From<Car> for PartialCar is provided.

Example 3: Multiple Partial Attributes

```
#[derive(Partial)]
  #[partial("UserInfo", derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq), omit(password))]
  #[partial("UserCreation", derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq), omit(id_user, password, registration_date, email_verified, user_rol))]
  pub struct User { ... }

```
    
This will generate two partial versions (UserInfo and UserCreation), each with its own conversion method.

How It Works
------------
When you derive Partial on a struct, the macro:
  1. Parses all #[partial(...)] attributes to determine the target names, extra derives, and which fields to omit.
  2. For each attribute, generates a new partial struct containing all fields from the full struct except those omitted.
  3. Implements a conversion method on the generated partial struct that takes the omitted fields as parameters and
     reconstructs the full struct.
  4. Implements From<FullStruct> for the generated partial struct, allowing conversion from the full struct via .into().

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

