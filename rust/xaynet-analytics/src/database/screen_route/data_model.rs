use chrono::{DateTime, Utc};
use isar_core::object::{data_type::DataType, object_builder::ObjectBuilder};
use std::vec::IntoIter;

use crate::database::common::{FieldProperty, IsarAdapter};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ScreenRoute {
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub object_id: Option<String>,
}

impl ScreenRoute {
    pub fn new<N: Into<String>>(name: N, created_at: DateTime<Utc>) -> ScreenRoute {
        ScreenRoute {
            name: name.into(),
            created_at,
            object_id: None,
        }
    }
}

impl IsarAdapter for ScreenRoute {
    fn into_field_properties() -> IntoIter<FieldProperty> {
        vec![
            FieldProperty::new("created_at".to_string(), DataType::String, None, None),
            FieldProperty::new("name".to_string(), DataType::String, None, None),
        ]
        .into_iter()
    }

    fn write_with_object_builder(&self, object_builder: &mut ObjectBuilder) {
        object_builder.write_string(Some(&self.created_at.to_rfc3339()));
        object_builder.write_string(Some(&self.name));
    }
}
