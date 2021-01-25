use anyhow::{anyhow, Error, Result};

use isar_core::{
    collection::IsarCollection,
    instance::IsarInstance,
    object::{object_builder::ObjectBuilder, object_id::ObjectId},
    schema::{collection_schema::CollectionSchema, Schema},
    txn::IsarTxn,
};
use std::vec::IntoIter;

use crate::database::{
    analytics_event::data_model::AnalyticsEvent,
    common::{FieldProperty, IsarAdapter},
};

pub struct IsarDb {
    instance: IsarInstance,
}

impl IsarDb {
    const MAX_SIZE: usize = 10000000;
    const ANALYTICS_EVENT_NAME: &'static str = "analytics_events";

    pub fn new(path: &str) -> Result<IsarDb, Error> {
        IsarInstance::create(path, IsarDb::MAX_SIZE, IsarDb::get_schema()?)
            .map_err(|_| anyhow!("failed to create IsarInstance"))
            .map(|instance| IsarDb { instance })
    }

    pub fn get_all_as_bytes(
        &self,
        collection_name: &str,
    ) -> Result<Vec<(&ObjectId, &[u8])>, Error> {
        let _bytes = self
            .instance
            .create_query_builder(self.get_collection(collection_name)?)
            .build()
            .find_all_vec(&self.begin_txn(false)?)
            .map_err(|_| {
                anyhow!(
                    "failed to find all bytes from collection {}",
                    collection_name
                )
            });

        // TODO: not sure how to proceed to parse [u8] using the collection schema. didn't find examples in Isar
        unimplemented!()
    }

    pub fn put(&self, collection_name: &str, object: &[u8]) -> Result<String, Error> {
        self.get_collection(collection_name)?
            .put(&self.begin_txn(false)?, None, object)
            .map_err(|_| {
                anyhow!(
                    "failed to add object {:?} to collection: {}",
                    object,
                    collection_name
                )
            })
            .map(|object_id| object_id.to_string())
    }

    pub fn get_object_builder(&self, collection_name: &str) -> Result<ObjectBuilder, Error> {
        Ok(self.get_collection(collection_name)?.get_object_builder())
    }

    fn get_schema() -> Result<Schema, Error> {
        let mut schema = Schema::new();
        schema
            .add_collection(get_collection_schema(
                Self::ANALYTICS_EVENT_NAME,
                &mut AnalyticsEvent::into_field_properties(),
            )?)
            .map_err(|_| {
                anyhow!(
                    "failed to add collection {} to schema",
                    Self::ANALYTICS_EVENT_NAME
                )
            })
            .map(|_| schema)
    }

    fn get_collection(&self, collection_name: &str) -> Result<&IsarCollection, Error> {
        self.instance
            .get_collection_by_name(collection_name)
            .ok_or_else(|| anyhow!("wrong collection name: {}", collection_name))
    }

    fn begin_txn(&self, write: bool) -> Result<IsarTxn, Error> {
        self.instance
            .begin_txn(write)
            .map_err(|_| anyhow!("failed to begin transaction"))
    }
}

fn get_collection_schema(
    name: &str,
    field_properties: &mut IntoIter<FieldProperty>,
) -> Result<CollectionSchema, Error> {
    field_properties.try_fold(CollectionSchema::new(&name), |mut schema, prop| {
        schema
            .add_property(&prop.name, prop.data_type)
            .map_err(|_| {
                anyhow!(
                    "failed to add property {} to collection {}",
                    prop.name,
                    name
                )
            })?;
        schema
            .add_index(&[&prop.name], prop.is_unique, prop.has_hash_value)
            .map_err(|_| {
                anyhow!(
                    "failed to add index for {} to collection {}",
                    prop.name,
                    name
                )
            })?;
        Ok(schema)
    })
}
