use chrono::{DateTime, Utc};
use isar_core::object::{data_type::DataType, object_builder::ObjectBuilder};
use std::vec::IntoIter;

use crate::database::{
    common::{FieldProperty, IsarAdapter},
    screen_route::data_model::ScreenRoute,
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum AnalyticsEventType {
    AppEvent = 0,
    Error = 1,
    ScreenEnter = 2,
    UserAction = 3,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct AnalyticsEvent<'a> {
    pub name: String,
    pub event_type: AnalyticsEventType,
    pub timestamp: DateTime<Utc>,
    pub screen_route: Option<&'a ScreenRoute>,
}

impl<'a> AnalyticsEvent<'a> {
    pub fn new<N: Into<String>>(
        name: N,
        event_type: AnalyticsEventType,
        timestamp: DateTime<Utc>,
        screen_route: Option<&'a ScreenRoute>,
    ) -> AnalyticsEvent {
        AnalyticsEvent {
            name: name.into(),
            event_type,
            timestamp,
            screen_route,
        }
    }

    fn write_screen_route(&self, object_builder: &mut ObjectBuilder) {
        match &self.screen_route {
            Some(screen) => match &screen.object_id {
                Some(object_id) => object_builder.write_string(Some(&object_id)),
                None => object_builder.write_null(),
            },
            None => object_builder.write_null(),
        }
    }
}

impl<'a> IsarAdapter for AnalyticsEvent<'a> {
    fn into_field_properties() -> IntoIter<FieldProperty> {
        // NOTE: properties need to be ordered by type. Properties with the same type need to be ordered alphabetically
        // https://github.com/isar/isar-core/blob/1ea9f27edfd6e3708daa47ac6a17995b628f31a6/src/schema/collection_schema.rs
        vec![
            FieldProperty::new("event_type".to_string(), DataType::Int, None, None),
            FieldProperty::new("name".to_string(), DataType::String, None, None),
            FieldProperty::new("screen_route".to_string(), DataType::String, None, None),
            FieldProperty::new("timestamp".to_string(), DataType::String, None, None),
        ]
        .into_iter()
    }

    fn write_with_object_builder(&self, object_builder: &mut ObjectBuilder) {
        object_builder.write_int(self.event_type as i32);
        object_builder.write_string(Some(&self.name));
        self.write_screen_route(object_builder);
        object_builder.write_string(Some(&self.timestamp.to_rfc3339()));
    }
}
