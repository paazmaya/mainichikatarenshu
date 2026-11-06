use serde::de::{Error as _, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::{collections::HashSet, error::Error, fmt};

#[derive(Deserialize)]
#[serde(untagged)]
enum Names {
    One(String),
    Many(Vec<String>),
}

struct Categories(Vec<String>);

impl<'de> Deserialize<'de> for Categories {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct CategoryVisitor;

        impl<'de> Visitor<'de> for CategoryVisitor {
            type Value = Categories;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a mapping of categories to kata names")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut categories = HashSet::new();
                let mut names = Vec::new();
                while let Some((category, value)) = map.next_entry::<String, Names>()? {
                    if category.trim().is_empty() || !categories.insert(category) {
                        return Err(M::Error::custom("empty or duplicate kata category"));
                    }
                    let group = match value {
                        Names::One(name) => vec![name],
                        Names::Many(names) => names,
                    };
                    if group.is_empty() {
                        return Err(M::Error::custom("kata categories cannot be empty"));
                    }
                    names.extend(group);
                }
                Ok(Categories(names))
            }
        }

        deserializer.deserialize_map(CategoryVisitor)
    }
}

pub fn parse(contents: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let Categories(names) = serde_saphyr::from_str(contents)?;
    if names.is_empty() {
        return Err("kata.yaml: at least one kata is required".into());
    }
    if names
        .iter()
        .any(|name| name.trim().is_empty() || name.len() > 64)
    {
        return Err("kata.yaml: kata names must contain 1 to 64 UTF-8 bytes".into());
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_existing_shape_and_multiple_names_in_order() {
        assert_eq!(
            parse("karate: tawata no passai\nkobujutsu: [tawata no sai, 型]").unwrap(),
            ["tawata no passai", "tawata no sai", "型"]
        );
    }

    #[test]
    fn rejects_invalid_configuration() {
        for contents in [
            "{}",
            "karate: []",
            "karate: '  '",
            "karate: null",
            "karate: [one, null]",
            "karate: one\nkarate: two",
            "'' : one",
            "- not a mapping",
        ] {
            assert!(parse(contents).is_err(), "{contents}");
        }
        assert!(parse(&format!("karate: '{}'", "a".repeat(65))).is_err());
    }
}
