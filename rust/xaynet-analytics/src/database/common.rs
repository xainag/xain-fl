use anyhow::Error;

use isar_core::object::{data_type::DataType, object_builder::ObjectBuilder};
use std::vec::IntoIter;

pub trait IsarAdapter: Sized {
    fn into_field_properties() -> IntoIter<FieldProperty>;

    fn write_with_object_builder(&self, object_builder: &mut ObjectBuilder);
}

pub trait Repo<T> {
    fn add(&self, object: &mut T) -> Result<(), Error>;

    fn get_all(&self) -> Result<Vec<T>, Error>;
}

pub struct MockRepo {}

pub struct MockObject {}

impl IsarAdapter for MockObject {
    fn into_field_properties() -> IntoIter<FieldProperty> {
        unimplemented!()
    }

    fn write_with_object_builder(&self, _object_builder: &mut ObjectBuilder) {
        unimplemented!()
    }
}

impl Repo<MockObject> for MockRepo {
    fn add(&self, _object: &mut MockObject) -> Result<(), Error> {
        unimplemented!()
    }

    fn get_all(&self) -> Result<Vec<MockObject>, Error> {
        unimplemented!()
    }
}

pub struct FieldProperty {
    pub name: String,
    pub data_type: DataType,
    pub is_unique: bool,
    pub has_hash_value: bool,
}

impl FieldProperty {
    pub fn new(
        name: String,
        data_type: DataType,
        is_unique: Option<bool>,
        has_hash_value: Option<bool>,
    ) -> Self {
        Self {
            name,
            data_type,
            is_unique: is_unique.unwrap_or(true),
            has_hash_value: has_hash_value.unwrap_or(false),
        }
    }
}
