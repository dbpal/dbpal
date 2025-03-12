use std::borrow::Cow;

use serde::Serializer;
use serde::ser::SerializeStruct;
/// meta value can used in various place.
#[derive()]
pub struct MetaValue {
    pub name: Cow<'static, str>,
    pub value: Option<Box<dyn ToString + Send + Sync + 'static>>,
    pub desc: Option<Cow<'static, str>>,
}

impl MetaValue {
    pub(crate) fn new<T: ToString + Sync + Send + 'static>(
        name: impl Into<Cow<'static, str>>, value: T,
    ) -> Self {
        Self {
            name: name.into(),
            value: Some(Box::new(value)),
            desc: None,
        }
    }

    pub(crate) fn new_opt<T: ToString + Sync + Send + 'static>(
        name: impl Into<Cow<'static, str>>, value: Option<T>,
    ) -> Self {
        Self {
            name: name.into(),
            value: value.map(|x| Box::new(x) as Box<dyn ToString + Sync + Send + 'static>),
            desc: None,
        }
    }

    pub(crate) fn with_desc(&mut self, desc: impl Into<Cow<'static, str>>) {
        self.desc = Some(desc.into());
    }
}

impl serde::Serialize for MetaValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_struct("MetaValue", 3)?;
        s.serialize_field("name", &self.name)?;
        match &self.value {
            Some(value) => s.serialize_field("value", &value.to_string())?,
            None => {}
        }
        match &self.desc {
            Some(x) => {
                s.serialize_field("desc", x)?;
            }
            None => {}
        }
        s.end()
    }
}

impl std::fmt::Debug for MetaValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[derive(Debug)]
        #[allow(dead_code)]
        pub struct MetaValue {
            name: Cow<'static, str>,
            value: Option<String>,
        }

        let m = MetaValue {
            name: self.name.clone(),
            value: match &self.value {
                Some(x) => Some(x.to_string()),
                None => None,
            },
        };
        write!(f, "{:?}", m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_meta_value() {
        let x = MetaValue::new("name", 1);
        assert_eq!(
            "MetaValue { name: \"name\", value: Some(\"1\") }",
            format!("{:?}", x)
        );
    }
}
